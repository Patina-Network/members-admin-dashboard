use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;

use super::response::AuthError;
use crate::{
    auth::session::{self, Principal},
    config::AuthConfig,
    http::state::AppState,
};

pub fn check_csrf(
    principal: &Principal,
    headers: &HeaderMap,
    config: &AuthConfig,
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

pub fn check_origin(headers: &HeaderMap, config: &AuthConfig) -> Result<(), AuthError> {
    if let Some(origin) = headers.get(header::ORIGIN) {
        let expected = config.origin.origin().ascii_serialization();
        if origin.to_str().ok() != Some(expected.as_str()) {
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

fn authorize(
    jar: &CookieJar,
    method: &Method,
    headers: &HeaderMap,
    config: &AuthConfig,
) -> Result<Principal, AuthError> {
    let principal = session::principal(jar, config).ok_or_else(AuthError::unauthorized)?;

    if !principal.is_admin(config) {
        return Err(AuthError::forbidden());
    }

    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        check_csrf(&principal, headers, config)?;
    }

    Ok(principal)
}

pub async fn protect(
    State(state): State<AppState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let database = is_database(path);
    let api = path == "/api" || path.starts_with("/api/");
    let callback = path == "/oauth2/callback";

    let mut response = if database || (api && !is_public_api(path)) {
        match authorize(
            &jar,
            request.method(),
            request.headers(),
            &state.auth.config,
        ) {
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

    if api || database || callback {
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
    use crate::{auth::session::round_trip, http::state::test_state};
    use axum::http::StatusCode;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn enforces_login_admin_and_csrf() {
        let config = test_state().config.auth;
        let mut headers = HeaderMap::new();
        let jar_for = |principal: &Principal| {
            round_trip(&session::establish(CookieJar::new(), &config, principal).unwrap())
        };

        let denied = authorize(&CookieJar::new(), &Method::GET, &headers, &config)
            .err()
            .unwrap();
        assert_eq!(denied.into_response().status(), StatusCode::UNAUTHORIZED);

        let mut principal = Principal {
            id: "test".into(),
            name: "Test".into(),
            email: Some("other@example.com".into()),
            csrf_token: "token".into(),
            expires_at: OffsetDateTime::now_utc().unix_timestamp() + 60,
        };
        assert!(matches!(
            authorize(&jar_for(&principal), &Method::GET, &headers, &config),
            Err(AuthError::Forbidden(_))
        ));

        principal.email = Some("admin@example.com".into());
        let jar = jar_for(&principal);
        assert!(authorize(&jar, &Method::GET, &headers, &config).is_ok());

        // Removing the email from the allowlist revokes an already-issued session.
        let mut revoked = config.clone();
        revoked.admin_emails.clear();
        assert!(matches!(
            authorize(&jar, &Method::GET, &headers, &revoked),
            Err(AuthError::Forbidden(_))
        ));
        let jar = jar_for(&principal);
        assert!(authorize(&jar, &Method::GET, &headers, &config).is_ok());

        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(matches!(
                authorize(&jar, &method, &headers, &config),
                Err(AuthError::Forbidden(_))
            ));
        }

        headers.insert("x-csrf-token", HeaderValue::from_static("token"));
        assert!(authorize(&jar, &Method::POST, &headers, &config).is_ok());

        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert!(matches!(
            authorize(&jar, &Method::POST, &headers, &config),
            Err(AuthError::Forbidden(_))
        ));
    }

    #[test]
    fn protects_member_paths_and_only_allows_exact_auth_routes() {
        for path in [
            "/api/members",
            "/api/members/42",
            "/api/suggestions/careers",
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
