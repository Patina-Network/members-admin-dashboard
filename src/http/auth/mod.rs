pub mod guard;
pub mod response;

use axum::{
    extract::{Query, State, rejection::QueryRejection},
    http::HeaderMap,
};
use serde::Deserialize;
use time::OffsetDateTime;
use tower_sessions::Session;

use crate::{
    auth::session::{self, LOGIN_KEY, LoginAttempt},
    http::state::AppState,
};
use response::{AuthError, FlowResponse, LogoutBody, LogoutResponse, SessionBody, SessionResponse};

#[utoipa::path(get, path = "/api/auth/login", responses(FlowResponse, AuthError))]
pub async fn login(
    State(state): State<AppState>,
    session: Session,
) -> Result<FlowResponse, AuthError> {
    let provider = state.auth.as_ref().ok_or_else(AuthError::unavailable)?;
    let (url, attempt) = match provider.begin().await {
        Ok(attempt) => attempt,
        Err(_) => {
            tracing::warn!("Azure authorization discovery failed");
            return Ok(FlowResponse::redirect("/login?error=unavailable"));
        }
    };
    session.insert(LOGIN_KEY, attempt).await?;
    session.save().await?;
    Ok(FlowResponse::redirect(url))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/auth/callback",
    params(CallbackQuery),
    responses(FlowResponse, AuthError)
)]
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    query: Result<Query<CallbackQuery>, QueryRejection>,
) -> Result<FlowResponse, AuthError> {
    let Query(query) = query.map_err(|_| AuthError::invalid())?;
    let provider = state.auth.as_ref().ok_or_else(AuthError::unavailable)?;
    let attempt: Option<LoginAttempt> = session.remove(LOGIN_KEY).await?;
    session.save().await?;
    let Some(attempt) = attempt.filter(|a| {
        query
            .state
            .as_deref()
            .is_some_and(|s| a.valid(s, OffsetDateTime::now_utc().unix_timestamp()))
    }) else {
        return Ok(FlowResponse::redirect("/login?error=invalid_callback"));
    };
    if query.error.is_some() {
        return Ok(FlowResponse::redirect("/login?error=cancelled"));
    }
    let Some(code) = query.code.filter(|s| !s.is_empty()) else {
        return Ok(FlowResponse::redirect("/login?error=invalid_callback"));
    };
    let principal = match provider.finish(code, attempt).await {
        Ok(principal) => principal,
        Err(_) => {
            tracing::warn!("Azure token exchange or validation failed");
            return Ok(FlowResponse::redirect("/login?error=login_failed"));
        }
    };
    complete_login(&session, principal).await
}

async fn complete_login(
    session: &Session,
    principal: crate::auth::session::Principal,
) -> Result<FlowResponse, AuthError> {
    if !principal.user.is_admin {
        session.flush().await?;
        return Ok(FlowResponse::redirect("/login?error=forbidden"));
    }
    session::establish(session, principal).await?;
    Ok(FlowResponse::redirect("/database"))
}

#[utoipa::path(get, path = "/api/auth/session", responses(SessionResponse, AuthError))]
pub async fn current_session(
    State(state): State<AppState>,
    session: Session,
) -> Result<SessionResponse, AuthError> {
    let principal = session::principal(&session).await?;
    Ok(SessionResponse::Success(SessionBody {
        user: principal.as_ref().map(|p| p.user.clone()),
        csrf_token: principal.map(|p| p.csrf_token),
        login_enabled: state.auth.is_some(),
    }))
}

#[utoipa::path(post, path = "/api/auth/logout", params(("X-CSRF-Token" = String, Header, description = "CSRF token from the current session")), responses(LogoutResponse, AuthError))]
pub async fn logout(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<LogoutResponse, AuthError> {
    guard::check_origin(&headers, state.auth.as_ref().map(|p| &p.config))?;
    let principal = session::principal(&session).await?;
    if let Some(principal) = principal {
        guard::check_csrf(&principal, &headers, state.auth.as_ref().map(|p| &p.config))?;
    }
    session.flush().await?;
    Ok(LogoutResponse::Success(LogoutBody {
        redirect_url: state
            .auth
            .as_ref()
            .map(|p| p.config.logout_url())
            .unwrap_or_else(|| "/login".into()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::{
            config::AuthConfig,
            provider::AzureProvider,
            session::{Principal, User},
        },
        http::state::test_state,
    };
    use axum::{
        http::{HeaderValue, StatusCode},
        response::IntoResponse,
    };
    use http_body_util::BodyExt;
    use openidconnect::ClientSecret;
    use std::sync::Arc;
    use tower_sessions::MemoryStore;

    fn auth_state() -> AppState {
        let mut state = test_state();
        state.auth = Some(Arc::new(
            AzureProvider::new(AuthConfig {
                tenant_id: "11111111-1111-1111-1111-111111111111".into(),
                client_id: "22222222-2222-2222-2222-222222222222".into(),
                client_secret: ClientSecret::new("test-secret".into()),
                origin: "http://localhost:8080".parse().unwrap(),
                admin_emails: vec!["admin@example.com".into()],
            })
            .unwrap(),
        ));
        state
    }

    fn principal() -> Principal {
        Principal {
            user: User {
                id: "user".into(),
                name: "User".into(),
                is_admin: true,
            },
            csrf_token: "csrf".into(),
            expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
        }
    }

    #[tokio::test]
    async fn login_only_establishes_sessions_for_allowlisted_accounts() {
        let store = Arc::new(MemoryStore::default());
        let session = Session::new(None, store.clone(), None);
        let response = complete_login(&session, principal())
            .await
            .unwrap_or_else(|_| panic!("login failed"))
            .into_response();
        assert_eq!(response.headers()["location"], "/database");
        assert!(session::principal(&session).await.unwrap().is_some());
        let old_id = session.id();
        let mut denied = principal();
        denied.user.is_admin = false;
        let response = complete_login(&session, denied)
            .await
            .unwrap_or_else(|_| panic!("login failed"))
            .into_response();
        assert_eq!(response.headers()["location"], "/login?error=forbidden");
        assert!(session::principal(&session).await.unwrap().is_none());
        assert!(
            session::principal(&Session::new(old_id, store, None))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn incomplete_callbacks_never_create_a_session() {
        for (state_value, code, has_attempt) in [
            (Some("expected"), None, true),
            (Some("expected"), Some(""), true),
            (None, Some("code"), true),
            (Some("expected"), Some("code"), false),
        ] {
            let session = Session::new(None, Arc::new(MemoryStore::default()), None);
            if has_attempt {
                session
                    .insert(
                        LOGIN_KEY,
                        LoginAttempt {
                            state: "expected".into(),
                            nonce: "nonce".into(),
                            verifier: "verifier".into(),
                            expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
                        },
                    )
                    .await
                    .unwrap();
            }
            let response = callback(
                State(auth_state()),
                session.clone(),
                Ok(Query(CallbackQuery {
                    code: code.map(str::to_owned),
                    state: state_value.map(str::to_owned),
                    error: None,
                })),
            )
            .await
            .unwrap_or_else(|_| panic!("callback failed"))
            .into_response();
            assert_eq!(
                response.headers()["location"],
                "/login?error=invalid_callback"
            );
            assert!(session::principal(&session).await.unwrap().is_none());
            assert!(
                session
                    .get::<LoginAttempt>(LOGIN_KEY)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[tokio::test]
    async fn malformed_callback_query_returns_bad_request() {
        use axum::extract::FromRequestParts;
        let (mut parts, _) = axum::http::Request::builder()
            .uri("/oauth2/callback?code=one&code=two")
            .body(())
            .unwrap()
            .into_parts();
        let query = Query::<CallbackQuery>::from_request_parts(&mut parts, &()).await;
        assert!(query.is_err());
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        let response = callback(State(auth_state()), session, query)
            .await
            .err()
            .unwrap()
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn reports_signed_out_session_when_auth_is_not_configured() {
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        let response = current_session(State(test_state()), session.clone())
            .await
            .unwrap_or_else(|_| panic!("session failed"))
            .into_response();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"user": null, "csrfToken": null, "loginEnabled": false})
        );
        assert!(matches!(
            login(State(test_state()), session).await,
            Err(AuthError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn logout_requires_csrf_then_revokes_the_session() {
        let store = Arc::new(MemoryStore::default());
        let session = Session::new(None, store.clone(), None);
        session::establish(&session, principal()).await.unwrap();
        let id = session.id();
        assert!(matches!(
            logout(State(auth_state()), session.clone(), HeaderMap::new()).await,
            Err(AuthError::Forbidden(_))
        ));
        assert!(session::principal(&session).await.unwrap().is_some());
        let mut headers = HeaderMap::new();
        headers.insert("x-csrf-token", HeaderValue::from_static("csrf"));
        headers.insert("origin", HeaderValue::from_static("http://localhost:8080"));
        let response = logout(State(auth_state()), session.clone(), headers)
            .await
            .unwrap_or_else(|_| panic!("logout failed"))
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(body["redirectUrl"].as_str().unwrap().starts_with("https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/oauth2/v2.0/logout?"));
        assert!(
            session::principal(&Session::new(id, store, None))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn callback_rejects_invalid_state_and_consumes_the_login_attempt() {
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        session
            .insert(
                LOGIN_KEY,
                LoginAttempt {
                    state: "expected".into(),
                    nonce: "nonce".into(),
                    verifier: "verifier".into(),
                    expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
                },
            )
            .await
            .unwrap();
        let query = CallbackQuery {
            code: Some("code".into()),
            state: Some("wrong".into()),
            error: None,
        };
        let response = callback(State(auth_state()), session.clone(), Ok(Query(query)))
            .await
            .unwrap_or_else(|_| panic!("callback failed"))
            .into_response();
        assert_eq!(
            response.headers()["location"],
            "/login?error=invalid_callback"
        );
        assert!(
            session
                .get::<LoginAttempt>(LOGIN_KEY)
                .await
                .unwrap()
                .is_none()
        );
        assert!(session::principal(&session).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn cancelled_login_returns_a_safe_message_without_creating_a_session() {
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        session
            .insert(
                LOGIN_KEY,
                LoginAttempt {
                    state: "expected".into(),
                    nonce: "nonce".into(),
                    verifier: "verifier".into(),
                    expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
                },
            )
            .await
            .unwrap();
        let query = CallbackQuery {
            code: None,
            state: Some("expected".into()),
            error: Some("access_denied".into()),
        };
        let response = callback(State(auth_state()), session.clone(), Ok(Query(query)))
            .await
            .unwrap_or_else(|_| panic!("callback failed"))
            .into_response();
        assert_eq!(response.headers()["location"], "/login?error=cancelled");
        assert!(session::principal(&session).await.unwrap().is_none());
    }
}
