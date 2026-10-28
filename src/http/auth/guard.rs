use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use super::response::AuthError;
use crate::{
    auth::{
        config::AuthConfig,
        session::{self, Principal},
    },
    http::state::AppState,
};

pub fn check_csrf(
    principal: &Principal,
    headers: &HeaderMap,
    config: Option<&AuthConfig>,
) -> Result<(), AuthError> {
    let token = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(AuthError::forbidden)?;
    if !principal.valid_csrf(token) {
        return Err(AuthError::forbidden());
    }
    check_origin(headers, config)
}

pub fn check_origin(headers: &HeaderMap, config: Option<&AuthConfig>) -> Result<(), AuthError> {
    if let Some(origin) = headers.get(header::ORIGIN) {
        let expected = config.map(|c| c.origin.origin().ascii_serialization());
        if origin.to_str().ok() != expected.as_deref() {
            return Err(AuthError::forbidden());
        }
    }
    Ok(())
}

fn is_database(path: &str) -> bool {
    path == "/database" || path.starts_with("/database/")
}

fn is_public_api(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/login"
            | "/api/auth/callback"
            | "/api/auth/session"
            | "/api/auth/logout"
            | "/api/openapi.json"
            | "/api/swagger-ui"
    ) || path.starts_with("/api/swagger-ui/")
}

pub async fn authorize(
    session: &Session,
    method: &Method,
    headers: &HeaderMap,
    config: Option<&AuthConfig>,
) -> Result<Principal, AuthError> {
    let principal = session::principal(session)
        .await?
        .ok_or_else(AuthError::unauthorized)?;
    if !principal.user.is_admin {
        return Err(AuthError::forbidden());
    }
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        check_csrf(&principal, headers, config)?;
    }
    Ok(principal)
}

pub async fn protect(
    State(state): State<AppState>,
    session: Session,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let database = is_database(path);
    let api = path == "/api" || path.starts_with("/api/");
    let mut response = if database || (api && !is_public_api(path)) {
        match authorize(
            &session,
            request.method(),
            request.headers(),
            state.auth.as_ref().map(|p| &p.config),
        )
        .await
        {
            Ok(principal) => {
                request.extensions_mut().insert(principal);
                next.run(request).await
            }
            Err(AuthError::Unauthorized(_)) if database => Redirect::to("/login").into_response(),
            Err(AuthError::Forbidden(_)) if database => {
                Redirect::to("/login?error=forbidden").into_response()
            }
            Err(error) => error.into_response(),
        }
    } else {
        next.run(request).await
    };
    if api || database {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response.headers_mut().insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::session::{PRINCIPAL_KEY, User};
    use axum::http::StatusCode;
    use std::sync::Arc;
    use time::OffsetDateTime;
    use tower_sessions::MemoryStore;

    #[tokio::test]
    async fn enforces_login_admin_and_csrf() {
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        let mut headers = HeaderMap::new();
        let denied = authorize(&session, &Method::GET, &headers, None)
            .await
            .err()
            .unwrap();
        assert_eq!(denied.into_response().status(), StatusCode::UNAUTHORIZED);
        let mut principal = Principal {
            user: User {
                id: "test".into(),
                name: "Test".into(),
                is_admin: false,
            },
            csrf_token: "token".into(),
            expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
        };
        session.insert(PRINCIPAL_KEY, &principal).await.unwrap();
        assert!(matches!(
            authorize(&session, &Method::GET, &headers, None).await,
            Err(AuthError::Forbidden(_))
        ));
        principal.user.is_admin = true;
        session.insert(PRINCIPAL_KEY, &principal).await.unwrap();
        assert!(
            authorize(&session, &Method::GET, &headers, None)
                .await
                .is_ok()
        );
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(matches!(
                authorize(&session, &method, &headers, None).await,
                Err(AuthError::Forbidden(_))
            ));
        }
        headers.insert("x-csrf-token", HeaderValue::from_static("token"));
        assert!(
            authorize(&session, &Method::POST, &headers, None)
                .await
                .is_ok()
        );
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert!(matches!(
            authorize(&session, &Method::POST, &headers, None).await,
            Err(AuthError::Forbidden(_))
        ));
    }

    #[test]
    fn protects_member_paths_and_only_allows_exact_auth_routes() {
        for path in [
            "/api/members",
            "/api/members/42",
            "/api/auth/other",
            "/api/auth/session/extra",
        ] {
            assert!(!is_public_api(path));
        }
        assert!(is_database("/database"));
        assert!(is_database("/database/members"));
        assert!(!is_database("/database-other"));
    }
}
