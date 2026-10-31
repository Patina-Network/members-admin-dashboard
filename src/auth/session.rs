use std::fmt;

use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, errors::Result};
use openidconnect::CsrfToken;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::Duration;

use crate::config::AuthConfig;

const SESSION_AUDIENCE: &str = "members-admin-session";
const LOGIN_AUDIENCE: &str = "members-admin-login";

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub is_admin: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub name: String,
    /// Validated Microsoft sign-in username (`preferred_username`).
    pub email: Option<String>,
    pub csrf_token: String,
    #[serde(rename = "exp")]
    pub expires_at: i64,
}

#[derive(Serialize, Deserialize)]
pub struct LoginAttempt {
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    #[serde(rename = "exp")]
    pub expires_at: i64,
}

impl LoginAttempt {
    pub fn valid(&self, state: &str, now: i64) -> bool {
        self.expires_at > now
            && CsrfToken::new(self.state.clone()) == CsrfToken::new(state.to_owned())
    }
}

impl Principal {
    pub fn valid_csrf(&self, token: &str) -> bool {
        CsrfToken::new(self.csrf_token.clone()) == CsrfToken::new(token.to_owned())
    }

    pub fn is_admin(&self, config: &AuthConfig) -> bool {
        config.allows_email(self.email.as_deref())
    }

    pub fn user(&self, config: &AuthConfig) -> User {
        User {
            id: self.id.clone(),
            name: self.name.clone(),
            is_admin: self.is_admin(config),
        }
    }
}

/// HS256 keys for session and login-attempt cookies.
#[derive(Clone)]
pub struct SessionKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    session: Validation,
    login: Validation,
}

impl SessionKeys {
    pub fn new(secret: &[u8]) -> Self {
        let validation = |audience: &str| {
            let mut validation = Validation::new(Algorithm::HS256);
            validation.leeway = 0;
            validation.set_audience(&[audience]);
            validation.set_required_spec_claims(&["exp", "aud"]);
            validation
        };

        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            session: validation(SESSION_AUDIENCE),
            login: validation(LOGIN_AUDIENCE),
        }
    }

    fn sign<T: Serialize>(&self, aud: &'static str, body: &T) -> Result<String> {
        #[derive(Serialize)]
        struct Claims<'a, T> {
            aud: &'static str,
            #[serde(flatten)]
            body: &'a T,
        }

        jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &Claims { aud, body },
            &self.encoding,
        )
    }

    fn verify<T: DeserializeOwned>(&self, token: &str, validation: &Validation) -> Option<T> {
        jsonwebtoken::decode(token, &self.decoding, validation)
            .ok()
            .map(|data| data.claims)
    }
}

impl fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionKeys(..)")
    }
}

fn session_cookie(config: &AuthConfig) -> &'static str {
    if config.secure_cookie() {
        "__Host-members_session"
    } else {
        "members_session"
    }
}

fn login_cookie(config: &AuthConfig) -> &'static str {
    if config.secure_cookie() {
        "__Host-members_login"
    } else {
        "members_login"
    }
}

fn cookie(
    name: &'static str,
    value: String,
    max_age: Duration,
    config: &AuthConfig,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .http_only(true)
        .secure(config.secure_cookie())
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(max_age)
        .build()
}

/// Signed-in principal from a valid, unexpired session cookie.
pub fn principal(jar: &CookieJar, config: &AuthConfig) -> Option<Principal> {
    let keys = &config.session_keys;
    keys.verify(jar.get(session_cookie(config))?.value(), &keys.session)
}

pub fn establish(jar: CookieJar, config: &AuthConfig, principal: &Principal) -> Result<CookieJar> {
    let token = config.session_keys.sign(SESSION_AUDIENCE, principal)?;
    Ok(jar.add(cookie(
        session_cookie(config),
        token,
        config.session_ttl,
        config,
    )))
}

pub fn clear(jar: CookieJar, config: &AuthConfig) -> CookieJar {
    jar.remove(cookie(
        session_cookie(config),
        String::new(),
        Duration::ZERO,
        config,
    ))
}

pub fn start_login(
    jar: CookieJar,
    config: &AuthConfig,
    attempt: &LoginAttempt,
) -> Result<CookieJar> {
    let token = config.session_keys.sign(LOGIN_AUDIENCE, attempt)?;
    Ok(jar.add(cookie(
        login_cookie(config),
        token,
        config.login_ttl,
        config,
    )))
}

/// Reads and always deletes the browser's login-attempt cookie. Copied tokens
/// remain valid until expiry.
pub fn take_login(jar: CookieJar, config: &AuthConfig) -> (CookieJar, Option<LoginAttempt>) {
    let keys = &config.session_keys;
    let attempt = jar
        .get(login_cookie(config))
        .and_then(|c| keys.verify(c.value(), &keys.login));

    (
        jar.remove(cookie(
            login_cookie(config),
            String::new(),
            Duration::ZERO,
            config,
        )),
        attempt,
    )
}

/// Replays a response jar as the next request's `Cookie` header, like a browser.
#[cfg(test)]
pub(crate) fn round_trip(jar: &CookieJar) -> CookieJar {
    let mut headers = axum::http::HeaderMap::new();
    for cookie in jar.iter() {
        headers.append(
            axum::http::header::COOKIE,
            format!("{}={}", cookie.name(), cookie.value())
                .parse()
                .unwrap(),
        );
    }
    CookieJar::from_headers(&headers)
}

#[cfg(test)]
pub(crate) fn test_keys() -> SessionKeys {
    SessionKeys::new(b"test-session-secret-with-32-bytes-or-more")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{app, state::test_state};
    use axum::{
        Router,
        body::Body,
        http::{Method, Request, StatusCode, header},
        response::IntoResponse,
    };
    use time::OffsetDateTime;
    use tower::ServiceExt;

    async fn request(
        router: Router,
        method: Method,
        path: &str,
        cookie: &str,
        csrf: &str,
    ) -> axum::response::Response {
        router
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::COOKIE, cookie)
                    .header("x-csrf-token", csrf)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn user(expires_at: i64) -> Principal {
        Principal {
            id: "user".into(),
            name: "User".into(),
            email: Some("admin@example.com".into()),
            csrf_token: "csrf".into(),
            expires_at,
        }
    }

    fn now() -> i64 {
        OffsetDateTime::now_utc().unix_timestamp()
    }

    fn session_header(config: &AuthConfig, principal: &Principal) -> String {
        let jar = establish(CookieJar::new(), config, principal).unwrap();
        let cookie = jar.get(session_cookie(config)).unwrap();
        format!("{}={}", cookie.name(), cookie.value())
    }

    fn authenticated_app(admin: bool) -> (Router, String) {
        let state = test_state();
        let mut principal = user(now() + 60);
        if !admin {
            principal.email = Some("other@example.com".into());
        }
        let cookie = session_header(&state.config.auth, &principal);

        (app(state), cookie)
    }

    #[tokio::test]
    async fn session_cookie_has_security_attributes_for_each_origin() {
        for origin in ["http://localhost:8080", "https://members.example.com"] {
            let mut config = test_state().config.auth;
            config.origin = origin.parse().unwrap();
            config.session_ttl = Duration::minutes(45);

            let response = establish(CookieJar::new(), &config, &user(now() + 60))
                .unwrap()
                .into_response();

            let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
            assert!(cookie.contains("HttpOnly"));
            assert!(cookie.contains("SameSite=Lax"));
            assert!(cookie.contains("Path=/"));
            assert!(cookie.contains("Max-Age=2700"));
            assert_eq!(cookie.contains("Secure"), config.secure_cookie());
            assert_eq!(
                cookie.starts_with("__Host-members_session="),
                config.secure_cookie()
            );
        }
    }

    #[tokio::test]
    async fn rejects_forged_expired_and_cross_purpose_tokens() {
        let config = test_state().config.auth;
        let valid = round_trip(&establish(CookieJar::new(), &config, &user(now() + 60)).unwrap());
        assert!(principal(&valid, &config).is_some());

        let mut other = config.clone();
        other.session_keys = SessionKeys::new(b"another-secret-with-32-bytes-or-more!!");
        assert!(principal(&valid, &other).is_none());

        let expired = round_trip(&establish(CookieJar::new(), &config, &user(now() - 1)).unwrap());
        assert!(principal(&expired, &config).is_none());

        let token = valid.get(session_cookie(&config)).unwrap().value();
        let (data, signature) = token.rsplit_once('.').unwrap();
        let replacement = if signature.starts_with('A') { "B" } else { "A" };
        let tampered = CookieJar::new().add(Cookie::new(
            session_cookie(&config),
            format!("{data}.{replacement}{}", &signature[1..]),
        ));
        assert!(principal(&tampered, &config).is_none());

        // A login-attempt token must never authenticate, even under the session cookie name.
        let attempt = LoginAttempt {
            state: "state".into(),
            nonce: "nonce".into(),
            verifier: "verifier".into(),
            expires_at: now() + 60,
        };
        let login = start_login(CookieJar::new(), &config, &attempt).unwrap();
        let login_token = login.get(login_cookie(&config)).unwrap().value().to_owned();
        let swapped =
            CookieJar::new().add(Cookie::new(session_cookie(&config), login_token.clone()));
        assert!(principal(&swapped, &config).is_none());

        let session_token = valid
            .get(session_cookie(&config))
            .unwrap()
            .value()
            .to_owned();
        let swapped = CookieJar::new().add(Cookie::new(login_cookie(&config), session_token));
        assert!(take_login(swapped, &config).1.is_none());

        let (jar, taken) = take_login(round_trip(&login), &config);
        assert!(taken.is_some());
        assert!(take_login(round_trip(&jar), &config).1.is_none());
    }

    #[tokio::test]
    async fn callback_routes_apply_auth_response_headers() {
        let router = app(test_state());

        for path in ["/oauth2/callback", "/api/auth/callback"] {
            let response = request(router.clone(), Method::GET, path, "", "").await;

            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        }
    }

    #[tokio::test]
    async fn http_routes_require_login_before_serving_database_or_member_api() {
        let router = app(test_state());

        for path in ["/", "/login", "/api/auth/session", "/livez"] {
            let response = request(router.clone(), Method::GET, path, "", "").await;
            assert_eq!(response.status(), StatusCode::OK, "{path}");
        }

        for path in ["/database", "/database/members"] {
            let response = request(router.clone(), Method::GET, path, "", "").await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()[header::LOCATION], "/login");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }

        for method in [Method::GET, Method::POST, Method::PATCH, Method::DELETE] {
            for path in ["/api/members", "/api/members/42"] {
                let response = request(router.clone(), method.clone(), path, "", "").await;
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            }
        }
    }

    #[tokio::test]
    async fn http_routes_enforce_admin_and_csrf_with_a_real_session_cookie() {
        let (router, cookie) = authenticated_app(false);

        let response = request(router.clone(), Method::GET, "/database", &cookie, "").await;
        assert_eq!(
            response.headers()[header::LOCATION],
            "/login?error=forbidden"
        );

        let response = request(router, Method::GET, "/api/members", &cookie, "").await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let (router, cookie) = authenticated_app(true);

        let response = request(router.clone(), Method::GET, "/database", &cookie, "").await;
        assert_eq!(response.status(), StatusCode::OK);

        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            let response = request(
                router.clone(),
                method.clone(),
                "/api/members",
                &cookie,
                "wrong",
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);

            let expected = if method == Method::POST {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::METHOD_NOT_ALLOWED
            };
            let response = request(router.clone(), method, "/api/members", &cookie, "csrf").await;
            assert_eq!(response.status(), expected);
        }
    }

    #[tokio::test]
    async fn http_logout_requires_csrf_and_deletes_the_cookie() {
        let (router, cookie) = authenticated_app(true);

        let response = request(
            router.clone(),
            Method::POST,
            "/api/auth/logout",
            &cookie,
            "wrong",
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = request(router, Method::POST, "/api/auth/logout", &cookie, "csrf").await;
        assert_eq!(response.status(), StatusCode::OK);

        let cookie_header = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie_header.starts_with("members_session=;"));
        assert!(cookie_header.contains("Max-Age=0"));
        assert!(cookie_header.contains("Path=/"));
    }

    #[tokio::test]
    async fn http_rejects_expired_principal_even_when_cookie_is_present() {
        let state = test_state();
        let cookie = session_header(&state.config.auth, &user(now() - 1));

        let response = request(app(state), Method::GET, "/api/members", &cookie, "").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn validates_state_expiry_and_csrf() {
        let attempt = LoginAttempt {
            state: "state".into(),
            nonce: "nonce".into(),
            verifier: "verifier".into(),
            expires_at: 100,
        };

        assert!(attempt.valid("state", 99));
        assert!(!attempt.valid("other", 99));
        assert!(!attempt.valid("state", 100));

        assert!(user(100).valid_csrf("csrf"));
        assert!(!user(100).valid_csrf("other"));
    }
}
