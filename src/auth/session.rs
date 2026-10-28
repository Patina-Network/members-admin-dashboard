use axum::Router;
use openidconnect::CsrfToken;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tower_sessions::{Expiry, Session, SessionManagerLayer, SessionStore, cookie::SameSite};

use super::{config::AuthConfig, store::ExpiringMemoryStore};

pub const PRINCIPAL_KEY: &str = "principal";
pub const LOGIN_KEY: &str = "login";

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub is_admin: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Principal {
    pub user: User,
    pub csrf_token: String,
    pub expires_at: i64,
}

#[derive(Serialize, Deserialize)]
pub struct LoginAttempt {
    pub state: String,
    pub nonce: String,
    pub verifier: String,
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
}

pub async fn principal(
    session: &Session,
) -> Result<Option<Principal>, tower_sessions::session::Error> {
    let principal: Option<Principal> = session.get(PRINCIPAL_KEY).await?;
    match principal {
        Some(principal) if principal.expires_at > OffsetDateTime::now_utc().unix_timestamp() => {
            Ok(Some(principal))
        }
        Some(_) => {
            session.flush().await?;
            Ok(None)
        }
        None => Ok(None),
    }
}

pub async fn establish(
    session: &Session,
    principal: Principal,
) -> Result<(), tower_sessions::session::Error> {
    session.flush().await?;
    session.cycle_id().await?;
    session.set_expiry(Some(Expiry::AtDateTime(
        OffsetDateTime::from_unix_timestamp(principal.expires_at).expect("validated token expiry"),
    )));
    session.insert(PRINCIPAL_KEY, principal).await?;
    session.save().await
}

fn layer<S: SessionStore>(store: S, secure: bool) -> SessionManagerLayer<S> {
    SessionManagerLayer::new(store)
        .with_name(if secure {
            "__Host-members_session"
        } else {
            "members_session"
        })
        .with_secure(secure)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_path("/")
        .with_expiry(Expiry::OnInactivity(Duration::minutes(10)))
}

pub fn with_sessions(router: Router, config: Option<&AuthConfig>) -> Router {
    let secure = config.is_some_and(AuthConfig::secure_cookie);
    tracing::warn!("sessions are stored in memory; restarting clears logins");
    router.layer(layer(ExpiringMemoryStore::default(), secure))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{app, state::test_state};
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header},
    };
    use std::sync::Arc;
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

    async fn authenticated_app(admin: bool) -> (Router, String, Session) {
        let store = ExpiringMemoryStore::default();
        let session = Session::new(None, Arc::new(store.clone()), None);
        let mut principal = user(OffsetDateTime::now_utc().unix_timestamp() + 60);
        principal.user.is_admin = admin;
        establish(&session, principal).await.unwrap();
        let cookie = format!("members_session={}", session.id().unwrap());
        let router = app(test_state()).layer(layer(store, false));
        (router, cookie, session)
    }

    #[tokio::test]
    async fn session_cookie_has_security_attributes_for_each_origin() {
        for origin in ["http://localhost:8080", "https://members.example.com"] {
            let config = AuthConfig {
                tenant_id: "11111111-1111-1111-1111-111111111111".into(),
                client_id: "22222222-2222-2222-2222-222222222222".into(),
                client_secret: openidconnect::ClientSecret::new("test".into()),
                origin: origin.parse().unwrap(),
                admin_emails: vec![],
            };
            let router = Router::new().route(
                "/",
                axum::routing::get(|session: Session| async move {
                    session.insert("value", "test").await.unwrap();
                    "ok"
                }),
            );
            let response = request(
                with_sessions(router, Some(&config)),
                Method::GET,
                "/",
                "",
                "",
            )
            .await;
            let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
            assert!(cookie.contains("HttpOnly"));
            assert!(cookie.contains("SameSite=Lax"));
            assert!(cookie.contains("Path=/"));
            assert_eq!(cookie.contains("Secure"), config.secure_cookie());
            assert_eq!(
                cookie.starts_with("__Host-members_session="),
                config.secure_cookie()
            );
        }
    }

    #[tokio::test]
    async fn callback_routes_apply_auth_response_headers() {
        let router = app(test_state()).layer(layer(ExpiringMemoryStore::default(), false));
        for path in ["/oauth2/callback", "/api/auth/callback"] {
            let response = request(router.clone(), Method::GET, path, "", "").await;
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        }
    }

    #[tokio::test]
    async fn http_routes_require_login_before_serving_database_or_member_api() {
        let router = app(test_state()).layer(layer(ExpiringMemoryStore::default(), false));
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
        let (router, cookie, _) = authenticated_app(false).await;
        let response = request(router.clone(), Method::GET, "/database", &cookie, "").await;
        assert_eq!(
            response.headers()[header::LOCATION],
            "/login?error=forbidden"
        );
        let response = request(router, Method::GET, "/api/members", &cookie, "").await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let (router, cookie, _) = authenticated_app(true).await;
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
            let response = request(router.clone(), method, "/api/members", &cookie, "csrf").await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn http_logout_deletes_cookie_and_rejects_replayed_session() {
        let (router, cookie, _) = authenticated_app(true).await;
        let response = request(
            router.clone(),
            Method::POST,
            "/api/auth/logout",
            &cookie,
            "wrong",
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = request(
            router.clone(),
            Method::POST,
            "/api/auth/logout",
            &cookie,
            "csrf",
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let cookie_header = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie_header.contains("members_session="));
        assert!(cookie_header.contains("Max-Age=0"));
        let response = request(router, Method::GET, "/api/members", &cookie, "").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn http_rejects_expired_principal_even_when_cookie_is_present() {
        let (router, cookie, session) = authenticated_app(true).await;
        session.insert(PRINCIPAL_KEY, user(0)).await.unwrap();
        session.save().await.unwrap();
        let response = request(router, Method::GET, "/api/members", &cookie, "").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    fn user(expires_at: i64) -> Principal {
        Principal {
            user: User {
                id: "user".into(),
                name: "User".into(),
                is_admin: true,
            },
            csrf_token: "csrf".into(),
            expires_at,
        }
    }

    #[tokio::test]
    async fn expires_and_revokes_sessions() {
        let store = Arc::new(ExpiringMemoryStore::default());
        let session = Session::new(None, store.clone(), None);
        establish(
            &session,
            user(OffsetDateTime::now_utc().unix_timestamp() + 60),
        )
        .await
        .unwrap();
        let id = session.id();
        assert!(
            principal(&Session::new(id, store.clone(), None))
                .await
                .unwrap()
                .is_some()
        );
        session.flush().await.unwrap();
        assert!(
            principal(&Session::new(id, store.clone(), None))
                .await
                .unwrap()
                .is_none()
        );
        session.insert(PRINCIPAL_KEY, user(0)).await.unwrap();
        assert!(principal(&session).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn login_replaces_the_old_session_id() {
        let store = Arc::new(ExpiringMemoryStore::default());
        let session = Session::new(None, store.clone(), None);
        session.insert("old", "value").await.unwrap();
        session.save().await.unwrap();
        let old_id = session.id();
        establish(
            &session,
            user(OffsetDateTime::now_utc().unix_timestamp() + 60),
        )
        .await
        .unwrap();
        assert_ne!(session.id(), old_id);
        assert!(
            Session::new(old_id, store, None)
                .get::<String>("old")
                .await
                .unwrap()
                .is_none()
        );
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
