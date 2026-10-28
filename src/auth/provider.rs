use anyhow::{Context, ensure};
use openidconnect::{
    AdditionalClaims, AuthType, AuthorizationCode, Client, ClientId, CsrfToken,
    EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet, IdTokenFields, IssuerUrl,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, StandardErrorResponse,
    StandardTokenResponse, TokenResponse, core::*, reqwest,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    config::AuthConfig,
    session::{LoginAttempt, Principal, User},
};

type AzureIdToken = openidconnect::IdToken<
    AzureClaims,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
>;

type AzureTokenResponse = StandardTokenResponse<
    IdTokenFields<
        AzureClaims,
        EmptyExtraTokenFields,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >,
    CoreTokenType,
>;
type AzureClient = Client<
    AzureClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    AzureTokenResponse,
    CoreTokenIntrospectionResponse,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AzureClaims {
    tid: String,
    #[serde(default)]
    roles: Vec<String>,
}
impl AdditionalClaims for AzureClaims {}

pub struct AzureProvider {
    pub config: AuthConfig,
    http: reqwest::Client,
}

impl AzureProvider {
    pub fn new(config: AuthConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { config, http })
    }

    async fn client(&self) -> anyhow::Result<AzureClient> {
        let issuer = IssuerUrl::new(format!(
            "https://login.microsoftonline.com/{}/v2.0",
            self.config.tenant_id
        ))?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &self.http).await?;
        Ok(AzureClient::from_provider_metadata(
            metadata,
            ClientId::new(self.config.client_id.clone()),
            Some(self.config.client_secret.clone()),
        )
        .set_auth_type(AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new(self.config.callback_url())?))
    }

    pub async fn begin(&self) -> anyhow::Result<(String, LoginAttempt)> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .add_extra_param("response_mode", "query")
            .url();
        Ok((
            url.into(),
            LoginAttempt {
                state: state.secret().clone(),
                nonce: nonce.secret().clone(),
                verifier: verifier.secret().clone(),
                expires_at: OffsetDateTime::now_utc().unix_timestamp() + 600,
            },
        ))
    }

    pub async fn finish(&self, code: String, attempt: LoginAttempt) -> anyhow::Result<Principal> {
        let client = self.client().await?;
        let token = client
            .exchange_code(AuthorizationCode::new(code))?
            .set_pkce_verifier(PkceCodeVerifier::new(attempt.verifier))
            .request_async(&self.http)
            .await?;
        let id_token = token
            .id_token()
            .context("Azure did not return an ID token")?;
        self.validate_id_token(&client, id_token, Nonce::new(attempt.nonce))
    }

    fn validate_id_token(
        &self,
        client: &AzureClient,
        id_token: &AzureIdToken,
        nonce: Nonce,
    ) -> anyhow::Result<Principal> {
        let claims = id_token.claims(&client.id_token_verifier(), &nonce)?;
        ensure!(
            claims.additional_claims().tid == self.config.tenant_id,
            "unexpected Azure tenant"
        );
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let expires_at = claims.expiration().timestamp().min(now + 3600);
        ensure!(expires_at > now, "expired Azure login");
        Ok(Principal {
            user: User {
                id: claims.subject().as_str().to_owned(),
                name: claims
                    .name()
                    .and_then(|n| n.get(None))
                    .map(|n| n.as_str())
                    .unwrap_or("Member")
                    .to_owned(),
                is_admin: claims
                    .additional_claims()
                    .roles
                    .contains(&self.config.admin_role),
            },
            csrf_token: CsrfToken::new_random().secret().clone(),
            expires_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use openidconnect::{
        Audience, ClientSecret, IdTokenClaims, JsonWebKeySet, PrivateSigningKey, StandardClaims,
        SubjectIdentifier,
    };
    use rsa::{RsaPrivateKey, pkcs1::EncodeRsaPrivateKey, rand_core::OsRng};

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const CLIENT: &str = "22222222-2222-2222-2222-222222222222";

    fn fixture() -> (AzureProvider, AzureClient, CoreRsaPrivateSigningKey) {
        let pem = RsaPrivateKey::new(&mut OsRng, 2048)
            .unwrap()
            .to_pkcs1_pem(Default::default())
            .unwrap();
        let key = CoreRsaPrivateSigningKey::from_pem(&pem, None).unwrap();
        let config = AuthConfig {
            tenant_id: TENANT.into(),
            client_id: CLIENT.into(),
            client_secret: ClientSecret::new("test".into()),
            origin: "http://localhost:8080".parse().unwrap(),
            admin_role: "Admin".into(),
        };
        let metadata: CoreProviderMetadata = serde_json::from_value(serde_json::json!({
            "issuer": format!("https://login.microsoftonline.com/{TENANT}/v2.0"),
            "authorization_endpoint": "https://login.microsoftonline.com/authorize",
            "token_endpoint": "https://login.microsoftonline.com/token",
            "jwks_uri": "https://login.microsoftonline.com/keys",
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        }))
        .unwrap();
        let client = AzureClient::from_provider_metadata(
            metadata.set_jwks(JsonWebKeySet::new(vec![key.as_verification_key()])),
            ClientId::new(CLIENT.into()),
            Some(config.client_secret.clone()),
        );
        (AzureProvider::new(config).unwrap(), client, key)
    }

    fn claims() -> IdTokenClaims<AzureClaims, CoreGenderClaim> {
        claims_with(AzureClaims {
            tid: TENANT.into(),
            roles: vec!["Admin".into()],
        })
    }

    fn claims_with(additional: AzureClaims) -> IdTokenClaims<AzureClaims, CoreGenderClaim> {
        IdTokenClaims::new(
            IssuerUrl::new(format!("https://login.microsoftonline.com/{TENANT}/v2.0")).unwrap(),
            vec![Audience::new(CLIENT.into())],
            Utc::now() + Duration::minutes(30),
            Utc::now(),
            StandardClaims::new(SubjectIdentifier::new("user-id".into())),
            additional,
        )
        .set_nonce(Some(Nonce::new("nonce".into())))
    }

    fn signed(
        claims: IdTokenClaims<AzureClaims, CoreGenderClaim>,
        key: &CoreRsaPrivateSigningKey,
    ) -> AzureIdToken {
        AzureIdToken::new(
            claims,
            key,
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn accepts_only_the_exact_admin_role_and_bounds_the_session() {
        let (provider, client, key) = fixture();
        for (roles, expected) in [
            (vec!["Admin".into()], true),
            (vec!["admin".into()], false),
            (vec![], false),
        ] {
            let token = signed(
                claims_with(AzureClaims {
                    tid: TENANT.into(),
                    roles,
                }),
                &key,
            );
            let principal = provider
                .validate_id_token(&client, &token, Nonce::new("nonce".into()))
                .unwrap();
            assert_eq!(principal.user.id, "user-id");
            assert_eq!(principal.user.is_admin, expected);
            assert!(principal.expires_at <= Utc::now().timestamp() + 1800);
            assert!(!principal.csrf_token.is_empty());
        }
    }

    #[test]
    fn rejects_invalid_issuer_audience_nonce_tenant_expiry_and_signature() {
        let (provider, client, key) = fixture();
        for claims in [
            claims().set_issuer(IssuerUrl::new("https://attacker.example".into()).unwrap()),
            claims().set_audiences(vec![Audience::new("wrong-client".into())]),
            claims().set_nonce(Some(Nonce::new("wrong-nonce".into()))),
            claims().set_expiration(Utc::now() - Duration::seconds(60)),
            claims_with(AzureClaims {
                tid: "other-tenant".into(),
                roles: vec!["Admin".into()],
            }),
        ] {
            assert!(
                provider
                    .validate_id_token(&client, &signed(claims, &key), Nonce::new("nonce".into()))
                    .is_err()
            );
        }
        let token = signed(claims(), &key).to_string();
        let (data, signature) = token.rsplit_once('.').unwrap();
        let replacement = if signature.starts_with('A') { "B" } else { "A" };
        let tampered: AzureIdToken = format!("{data}.{replacement}{}", &signature[1..])
            .parse()
            .unwrap();
        assert!(
            provider
                .validate_id_token(&client, &tampered, Nonce::new("nonce".into()))
                .is_err()
        );
    }
}
