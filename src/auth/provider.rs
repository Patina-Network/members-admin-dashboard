use anyhow::{Context, bail, ensure};
use openidconnect::{
    AdditionalClaims, AuthType, AuthorizationCode, Client, ClientId, CsrfToken,
    EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet, IdTokenFields, IssuerUrl,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, StandardErrorResponse,
    StandardTokenResponse, TokenResponse, core::*, reqwest,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
use tokio::{sync::Mutex, time::Instant};

use super::session::{LoginAttempt, Principal, User};
use crate::config::AuthConfig;

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
}
impl AdditionalClaims for AzureClaims {}

const DISCOVERY_TTL: Duration = Duration::from_secs(3600);
const DISCOVERY_RETRY_DELAY: Duration = Duration::from_secs(30);
const KEY_REFRESH_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Default)]
struct ClientCache {
    client: Option<(Arc<AzureClient>, Instant)>,
    retry_after: Option<Instant>,
    key_refresh_after: Option<Instant>,
}

pub struct AzureProvider<H = reqwest::Client> {
    pub config: AuthConfig,
    http: H,
    cache: Mutex<ClientCache>,
}

impl AzureProvider {
    pub fn new(config: AuthConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            config,
            http,
            cache: Mutex::default(),
        })
    }
}

impl<H> AzureProvider<H>
where
    H: for<'c> openidconnect::AsyncHttpClient<'c>,
    for<'c> <H as openidconnect::AsyncHttpClient<'c>>::Error: Send + Sync,
{
    async fn client(&self) -> anyhow::Result<Arc<AzureClient>> {
        self.cached_client(None).await
    }

    // Hold one async lock through discovery so concurrent requests share a refresh.
    async fn cached_client(
        &self,
        stale: Option<&Arc<AzureClient>>,
    ) -> anyhow::Result<Arc<AzureClient>> {
        let mut cache = self.cache.lock().await;
        let now = Instant::now();

        if let Some((client, expires_at)) = &cache.client
            && *expires_at > now
            && (stale.is_none() || stale.is_some_and(|old| !Arc::ptr_eq(old, client)))
        {
            return Ok(client.clone());
        }

        if cache.retry_after.is_some_and(|at| at > now) {
            if stale.is_none()
                && let Some((client, _)) = &cache.client
            {
                return Ok(client.clone());
            }
            bail!("Azure discovery temporarily unavailable");
        }

        if stale.is_some() {
            ensure!(
                cache.key_refresh_after.is_none_or(|at| at <= now),
                "Azure signing key refresh rate limited"
            );
            cache.key_refresh_after = Some(now + KEY_REFRESH_INTERVAL);
        }

        // Also back off after cancellation or failure of a discovery request.
        cache.retry_after = Some(now + DISCOVERY_RETRY_DELAY);
        let client = Arc::new(self.discover_client().await?);
        cache.client = Some((client.clone(), Instant::now() + DISCOVERY_TTL));
        cache.retry_after = None;

        Ok(client)
    }

    async fn discover_client(&self) -> anyhow::Result<AzureClient> {
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
        let nonce = Nonce::new(attempt.nonce);

        match self.validate_id_token(&client, id_token, nonce.clone()) {
            Err(error)
                if matches!(
                    error.downcast_ref::<openidconnect::ClaimsVerificationError>(),
                    Some(
                        openidconnect::ClaimsVerificationError::SignatureVerification(
                            openidconnect::SignatureVerificationError::NoMatchingKey
                        )
                    )
                ) =>
            {
                let refreshed = self.cached_client(Some(&client)).await?;
                self.validate_id_token(&refreshed, id_token, nonce)
            }
            result => result,
        }
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

        let is_admin = self.config.allows_email(
            claims
                .preferred_username()
                .map(|username| username.as_str()),
        );

        Ok(Principal {
            user: User {
                id: claims.subject().as_str().to_owned(),
                name: claims
                    .name()
                    .and_then(|n| n.get(None))
                    .map(|n| n.as_str())
                    .unwrap_or("Member")
                    .to_owned(),
                is_admin,
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
        Audience, ClientSecret, EndUserEmail, EndUserUsername, IdTokenClaims, JsonWebKeySet,
        PrivateSigningKey, StandardClaims, SubjectIdentifier,
    };
    use rsa::{RsaPrivateKey, pkcs1::EncodeRsaPrivateKey, rand_core::OsRng};

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const CLIENT: &str = "22222222-2222-2222-2222-222222222222";

    struct MockHttp {
        jwks: serde_json::Value,
        token: std::sync::Mutex<(u16, serde_json::Value)>,
        requests: std::sync::Mutex<Vec<openidconnect::HttpRequest>>,
        discovery_status: u16,
    }

    impl<'c> openidconnect::AsyncHttpClient<'c> for MockHttp {
        type Error = std::io::Error;
        type Future = std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<openidconnect::HttpResponse, Self::Error>>
                    + Send
                    + 'c,
            >,
        >;

        fn call(&'c self, request: openidconnect::HttpRequest) -> Self::Future {
            let (status, body) = match request.uri().path() {
                "/keys" => (200, self.jwks.clone()),
                "/token" => self.token.lock().unwrap().clone(),
                path if path.ends_with("/.well-known/openid-configuration") => (
                    self.discovery_status,
                    serde_json::json!({
                        "issuer": format!("https://login.microsoftonline.com/{TENANT}/v2.0"),
                        "authorization_endpoint": "https://login.microsoftonline.com/authorize",
                        "token_endpoint": "https://login.microsoftonline.com/token",
                        "jwks_uri": "https://login.microsoftonline.com/keys",
                        "response_types_supported": ["code"],
                        "subject_types_supported": ["public"],
                        "id_token_signing_alg_values_supported": ["RS256"]
                    }),
                ),
                path => panic!("unexpected OAuth request: {path}"),
            };

            self.requests.lock().unwrap().push(request);
            let response = openidconnect::http::Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(serde_json::to_vec(&body).unwrap())
                .unwrap();

            Box::pin(async move {
                tokio::task::yield_now().await;
                Ok(response)
            })
        }
    }

    fn mock_provider(
        config: AuthConfig,
        key: &CoreRsaPrivateSigningKey,
    ) -> AzureProvider<MockHttp> {
        AzureProvider {
            config,
            cache: Mutex::default(),
            http: MockHttp {
                jwks: serde_json::to_value(JsonWebKeySet::new(vec![key.as_verification_key()]))
                    .unwrap(),
                token: std::sync::Mutex::new((200, serde_json::json!({}))),
                requests: std::sync::Mutex::new(Vec::new()),
                discovery_status: 200,
            },
        }
    }

    fn discovery_count(provider: &AzureProvider<MockHttp>) -> usize {
        provider
            .http
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                r.uri()
                    .path()
                    .ends_with("/.well-known/openid-configuration")
            })
            .count()
    }

    fn token_response(
        provider: &AzureProvider<MockHttp>,
        key: &CoreRsaPrivateSigningKey,
        nonce: &str,
    ) {
        *provider.http.token.lock().unwrap() = (
            200,
            serde_json::json!({
                "access_token": "test", "token_type": "Bearer",
                "id_token": signed(claims().set_nonce(Some(Nonce::new(nonce.into()))), key).to_string()
            }),
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shares_discovery_and_refreshes_expired_clients_with_failure_backoff() {
        let (original, _, key) = fixture();
        let mut provider = mock_provider(original.config, &key);
        let (first, second, third) =
            tokio::join!(provider.client(), provider.client(), provider.client());

        let first = first.unwrap();
        assert!(Arc::ptr_eq(&first, &second.unwrap()));
        assert!(Arc::ptr_eq(&first, &third.unwrap()));
        assert_eq!(discovery_count(&provider), 1);

        provider.begin().await.unwrap();
        assert_eq!(discovery_count(&provider), 1);

        tokio::time::advance(DISCOVERY_TTL).await;
        let second = provider.client().await.unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(discovery_count(&provider), 2);

        tokio::time::advance(DISCOVERY_TTL).await;
        provider.http.discovery_status = 503;
        let (a, b) = tokio::join!(provider.client(), provider.client());
        let served = [a, b]
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        assert_eq!(served.len(), 1);
        assert!(Arc::ptr_eq(&served[0], &second));
        assert_eq!(discovery_count(&provider), 3);

        provider.http.discovery_status = 200;
        let during_backoff = provider.client().await.unwrap();
        assert!(Arc::ptr_eq(&during_backoff, &second));
        assert!(provider.cached_client(Some(&second)).await.is_err());
        assert_eq!(discovery_count(&provider), 3);

        tokio::time::advance(DISCOVERY_RETRY_DELAY).await;
        let refreshed = provider.client().await.unwrap();
        assert!(!Arc::ptr_eq(&refreshed, &second));
        assert_eq!(discovery_count(&provider), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn discovery_failure_without_a_client_stays_unavailable() {
        let (original, _, key) = fixture();
        let mut provider = mock_provider(original.config, &key);

        provider.http.discovery_status = 503;
        assert!(provider.client().await.is_err());

        provider.http.discovery_status = 200;
        assert!(provider.client().await.is_err());
        assert_eq!(discovery_count(&provider), 1);

        tokio::time::advance(DISCOVERY_RETRY_DELAY).await;
        assert!(provider.client().await.is_ok());
        assert_eq!(discovery_count(&provider), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn refreshes_unknown_keys_once_without_redeeming_the_code_again() {
        let (original, _, key) = fixture();
        let mut provider = mock_provider(original.config, &key);
        let (_, attempt) = provider.begin().await.unwrap();

        let rotated = signing_key("rotated-key");
        provider.http.jwks = serde_json::to_value(JsonWebKeySet::new(vec![
            rotated.as_verification_key(),
            key.as_verification_key(),
        ]))
        .unwrap();
        token_response(&provider, &rotated, &attempt.nonce);

        assert!(
            provider
                .finish("code".into(), attempt)
                .await
                .unwrap()
                .user
                .is_admin
        );
        assert_eq!(discovery_count(&provider), 2);
        assert_eq!(
            provider
                .http
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.uri().path() == "/token")
                .count(),
            1
        );

        let (_, attempt) = provider.begin().await.unwrap();
        token_response(&provider, &rotated, "wrong-nonce");
        assert!(provider.finish("code".into(), attempt).await.is_err());
        assert_eq!(discovery_count(&provider), 2);

        let unknown = signing_key("unknown-key");

        for _ in 0..2 {
            let (_, attempt) = provider.begin().await.unwrap();
            token_response(&provider, &unknown, &attempt.nonce);
            assert!(provider.finish("code".into(), attempt).await.is_err());
        }

        assert_eq!(discovery_count(&provider), 2);

        tokio::time::advance(KEY_REFRESH_INTERVAL).await;
        let (_, attempt) = provider.begin().await.unwrap();
        token_response(&provider, &unknown, &attempt.nonce);
        assert!(provider.finish("code".into(), attempt).await.is_err());
        assert_eq!(discovery_count(&provider), 3);
    }

    #[tokio::test]
    async fn concurrent_key_refreshes_share_one_discovery() {
        let (original, _, key) = fixture();
        let provider = mock_provider(original.config, &key);

        let old = provider.client().await.unwrap();
        let (a, b) = tokio::join!(
            provider.cached_client(Some(&old)),
            provider.cached_client(Some(&old))
        );

        assert!(Arc::ptr_eq(&a.unwrap(), &b.unwrap()));
        assert_eq!(discovery_count(&provider), 2);
    }

    #[tokio::test]
    async fn authorization_code_flow_sends_pkce_and_validates_the_token_response() {
        let (original, _, key) = fixture();
        let mut provider = mock_provider(original.config, &key);
        let (url, attempt) = provider.begin().await.unwrap();
        let url = openidconnect::url::Url::parse(&url).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

        assert_eq!(params["client_id"], CLIENT);
        assert_eq!(params["response_type"], "code");
        assert_eq!(params["scope"], "openid profile");
        assert_eq!(params["state"], attempt.state);
        assert_eq!(params["nonce"], attempt.nonce);
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(
            params["code_challenge"],
            PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
                attempt.verifier.clone()
            ))
            .as_str()
        );
        assert_eq!(params["redirect_uri"], provider.config.callback_url());

        let verifier = attempt.verifier.clone();
        *provider.http.token.lock().unwrap() = (
            200,
            serde_json::json!({
                "access_token": "test-access-token", "token_type": "Bearer",
                "id_token": signed(claims().set_nonce(Some(Nonce::new(attempt.nonce.clone()))), &key).to_string()
            }),
        );

        let principal = provider
            .finish("authorization-code".into(), attempt)
            .await
            .unwrap();

        assert!(principal.user.is_admin);
        assert_eq!(principal.user.id, "user-id");

        {
            let requests = provider.http.requests.lock().unwrap();
            let request = requests.last().unwrap();
            assert_eq!(request.method(), "POST");
            let form: std::collections::HashMap<_, _> =
                openidconnect::url::form_urlencoded::parse(request.body())
                    .into_owned()
                    .collect();

            assert_eq!(form["code"], "authorization-code");
            assert_eq!(form["code_verifier"], verifier);
            assert_eq!(form["client_secret"], "test");
            assert_eq!(form["grant_type"], "authorization_code");
        }

        for (status, body) in [
            (
                200,
                serde_json::json!({"access_token": "test", "token_type": "Bearer"}),
            ),
            (400, serde_json::json!({"error": "invalid_grant"})),
            (503, serde_json::json!({"error": "temporarily_unavailable"})),
        ] {
            let (_, attempt) = provider.begin().await.unwrap();
            *provider.http.token.lock().unwrap() = (status, body);
            assert!(provider.finish("code".into(), attempt).await.is_err());
        }

        provider.http.discovery_status = 503;
        provider.cache.lock().await.client = None;
        assert!(provider.begin().await.is_err());
    }

    fn signing_key(kid: &str) -> CoreRsaPrivateSigningKey {
        let pem = RsaPrivateKey::new(&mut OsRng, 2048)
            .unwrap()
            .to_pkcs1_pem(Default::default())
            .unwrap();

        CoreRsaPrivateSigningKey::from_pem(&pem, Some(openidconnect::JsonWebKeyId::new(kid.into())))
            .unwrap()
    }

    fn fixture() -> (AzureProvider, AzureClient, CoreRsaPrivateSigningKey) {
        let key = signing_key("initial-key");

        let config = AuthConfig {
            tenant_id: TENANT.into(),
            client_id: CLIENT.into(),
            client_secret: ClientSecret::new("test".into()),
            origin: "http://localhost:8080".parse().unwrap(),
            admin_emails: vec!["admin@example.com".into()],
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
        claims_with(AzureClaims { tid: TENANT.into() })
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
        .set_preferred_username(Some(EndUserUsername::new("admin@example.com".into())))
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
        .to_string()
        .parse()
        .unwrap()
    }

    #[test]
    fn accepts_only_allowlisted_sign_in_emails_and_bounds_the_session() {
        let (provider, client, key) = fixture();

        for (preferred_username, expected) in [
            (Some("admin@example.com".into()), true),
            (Some("ADMIN@EXAMPLE.COM".into()), true),
            (Some("other@example.com".into()), false),
            (Some("admin@example.com.attacker.test".into()), false),
            (None, false),
        ] {
            let token = signed(
                claims_with(AzureClaims { tid: TENANT.into() })
                    .set_preferred_username(preferred_username.map(EndUserUsername::new))
                    .set_email(Some(EndUserEmail::new("admin@example.com".into()))),
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
