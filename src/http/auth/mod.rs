pub mod guard;
pub mod response;

use axum::{
    extract::{Query, State, rejection::QueryRejection},
    http::HeaderMap,
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::{auth::session, config::AuthConfig, http::state::AppState};
use response::{AuthError, FlowResponse, LogoutBody, LogoutResponse, SessionBody, SessionResponse};

#[utoipa::path(get, path = "/api/auth/login", responses(FlowResponse, AuthError))]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, FlowResponse), AuthError> {
    let (url, attempt) = match state.auth.begin().await {
        Ok(attempt) => attempt,
        Err(_) => {
            tracing::warn!("Azure authorization discovery failed");
            return Ok((jar, FlowResponse::redirect("/login?error=unavailable")));
        }
    };

    let jar = session::start_login(jar, &state.auth.config, &attempt)?;
    Ok((jar, FlowResponse::redirect(url)))
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
    jar: CookieJar,
    query: Result<Query<CallbackQuery>, QueryRejection>,
) -> Result<(CookieJar, FlowResponse), AuthError> {
    let Query(query) = query.map_err(|_| AuthError::invalid())?;
    let config = &state.auth.config;

    let (jar, attempt) = session::take_login(jar, config);

    let Some(attempt) = attempt.filter(|a| {
        query
            .state
            .as_deref()
            .is_some_and(|s| a.valid(s, OffsetDateTime::now_utc().unix_timestamp()))
    }) else {
        return Ok((jar, FlowResponse::redirect("/login?error=invalid_callback")));
    };

    if query.error.is_some() {
        return Ok((jar, FlowResponse::redirect("/login?error=cancelled")));
    }

    let Some(code) = query.code.filter(|s| !s.is_empty()) else {
        return Ok((jar, FlowResponse::redirect("/login?error=invalid_callback")));
    };

    let principal = match state.auth.finish(code, attempt).await {
        Ok(principal) => principal,
        Err(_) => {
            tracing::warn!("Azure token exchange or validation failed");
            return Ok((jar, FlowResponse::redirect("/login?error=login_failed")));
        }
    };

    complete_login(jar, config, principal)
}

fn complete_login(
    jar: CookieJar,
    config: &AuthConfig,
    principal: session::Principal,
) -> Result<(CookieJar, FlowResponse), AuthError> {
    if !principal.user.is_admin {
        return Ok((
            session::clear(jar, config),
            FlowResponse::redirect("/login?error=forbidden"),
        ));
    }

    let jar = session::establish(jar, config, &principal)?;
    Ok((jar, FlowResponse::redirect("/database")))
}

#[utoipa::path(get, path = "/api/auth/session", responses(SessionResponse))]
pub async fn current_session(State(state): State<AppState>, jar: CookieJar) -> SessionResponse {
    let principal = session::principal(&jar, &state.auth.config);
    SessionResponse::Success(SessionBody {
        user: principal.as_ref().map(|p| p.user.clone()),
        csrf_token: principal.map(|p| p.csrf_token),
    })
}

#[utoipa::path(post, path = "/api/auth/logout", params(("X-CSRF-Token" = String, Header, description = "CSRF token from the current session")), responses(LogoutResponse, AuthError))]
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<(CookieJar, LogoutResponse), AuthError> {
    let config = &state.auth.config;
    guard::check_origin(&headers, config)?;

    if let Some(principal) = session::principal(&jar, config) {
        guard::check_csrf(&principal, &headers, config)?;
    }

    Ok((
        session::clear(jar, config),
        LogoutResponse::Success(LogoutBody {
            redirect_url: config.logout_url(),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::session::{LoginAttempt, Principal, User, round_trip},
        http::state::test_state,
    };
    use axum::{
        http::{HeaderValue, StatusCode, header},
        response::IntoResponse,
    };
    use http_body_util::BodyExt;

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

    fn login_jar(config: &AuthConfig) -> CookieJar {
        let attempt = LoginAttempt {
            state: "expected".into(),
            nonce: "nonce".into(),
            verifier: "verifier".into(),
            expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
        };
        round_trip(&session::start_login(CookieJar::new(), config, &attempt).unwrap())
    }

    fn query(code: Option<&str>, state: Option<&str>, error: Option<&str>) -> CallbackQuery {
        CallbackQuery {
            code: code.map(str::to_owned),
            state: state.map(str::to_owned),
            error: error.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn login_only_establishes_sessions_for_allowlisted_accounts() {
        let config = test_state().config.auth;

        let (jar, response) = complete_login(CookieJar::new(), &config, principal())
            .unwrap_or_else(|_| panic!("login failed"));
        assert_eq!(response.into_response().headers()["location"], "/database");
        let jar = round_trip(&jar);
        assert!(session::principal(&jar, &config).is_some());

        let mut denied = principal();
        denied.user.is_admin = false;

        let (jar, response) =
            complete_login(jar, &config, denied).unwrap_or_else(|_| panic!("login failed"));
        assert_eq!(
            response.into_response().headers()["location"],
            "/login?error=forbidden"
        );
        assert!(session::principal(&round_trip(&jar), &config).is_none());
    }

    #[tokio::test]
    async fn incomplete_callbacks_never_create_a_session() {
        for (state_value, code, has_attempt) in [
            (Some("expected"), None, true),
            (Some("expected"), Some(""), true),
            (None, Some("code"), true),
            (Some("expected"), Some("code"), false),
        ] {
            let state = test_state();
            let config = state.auth.config.clone();
            let jar = if has_attempt {
                login_jar(&config)
            } else {
                CookieJar::new()
            };

            let (jar, response) =
                callback(State(state), jar, Ok(Query(query(code, state_value, None))))
                    .await
                    .unwrap_or_else(|_| panic!("callback failed"));

            assert_eq!(
                response.into_response().headers()["location"],
                "/login?error=invalid_callback"
            );
            let jar = round_trip(&jar);
            assert!(session::principal(&jar, &config).is_none());
            assert!(session::take_login(jar, &config).1.is_none());
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

        let response = callback(State(test_state()), CookieJar::new(), query)
            .await
            .err()
            .unwrap()
            .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn reports_signed_out_session() {
        let response = current_session(State(test_state()), CookieJar::new())
            .await
            .into_response();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"user": null, "csrfToken": null})
        );
    }

    #[tokio::test]
    async fn logout_requires_csrf_then_clears_the_session_cookie() {
        let state = test_state();
        let jar = round_trip(
            &session::establish(CookieJar::new(), &state.auth.config, &principal()).unwrap(),
        );

        assert!(matches!(
            logout(State(state.clone()), jar.clone(), HeaderMap::new()).await,
            Err(AuthError::Forbidden(_))
        ));

        let mut headers = HeaderMap::new();
        headers.insert("x-csrf-token", HeaderValue::from_static("csrf"));
        headers.insert("origin", HeaderValue::from_static("http://localhost:8080"));

        let response = logout(State(state), jar, headers)
            .await
            .unwrap_or_else(|_| panic!("logout failed"))
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.starts_with("members_session=;"));
        assert!(cookie.contains("Max-Age=0"));

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(body["redirectUrl"].as_str().unwrap().starts_with("https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/oauth2/v2.0/logout?"));
    }

    #[tokio::test]
    async fn callback_rejects_invalid_state_and_consumes_the_login_attempt() {
        let state = test_state();
        let config = state.auth.config.clone();

        let (jar, response) = callback(
            State(state),
            login_jar(&config),
            Ok(Query(query(Some("code"), Some("wrong"), None))),
        )
        .await
        .unwrap_or_else(|_| panic!("callback failed"));

        let response = (jar.clone(), response).into_response();
        assert_eq!(
            response.headers()["location"],
            "/login?error=invalid_callback"
        );
        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.starts_with("members_login=;"));
        assert!(cookie.contains("Max-Age=0"));

        let jar = round_trip(&jar);
        assert!(session::principal(&jar, &config).is_none());
        assert!(session::take_login(jar, &config).1.is_none());
    }

    #[tokio::test]
    async fn cancelled_login_returns_a_safe_message_without_creating_a_session() {
        let state = test_state();
        let config = state.auth.config.clone();

        let (jar, response) = callback(
            State(state),
            login_jar(&config),
            Ok(Query(query(None, Some("expected"), Some("access_denied")))),
        )
        .await
        .unwrap_or_else(|_| panic!("callback failed"));

        assert_eq!(
            response.into_response().headers()["location"],
            "/login?error=cancelled"
        );
        assert!(session::principal(&round_trip(&jar), &config).is_none());
    }
}
