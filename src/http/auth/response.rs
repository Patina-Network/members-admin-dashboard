use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Serialize;
use utoipa::{IntoResponses, ToResponse, ToSchema};

use crate::{auth::session::User, http::ErrorBody};

#[derive(IntoResponses)]
pub enum AuthError {
    #[response(status = 400, description = "Invalid authentication request")]
    Invalid(ErrorBody),
    #[response(status = 401, description = "Authentication required")]
    Unauthorized(ErrorBody),
    #[response(status = 403, description = "Admin access or CSRF validation denied")]
    Forbidden(ErrorBody),
    #[response(status = 500, description = "Session operation failed")]
    Internal(ErrorBody),
}

impl AuthError {
    pub fn unauthorized() -> Self {
        Self::Unauthorized(ErrorBody {
            error: "authentication required",
        })
    }

    pub fn forbidden() -> Self {
        Self::Forbidden(ErrorBody {
            error: "access denied",
        })
    }

    pub fn invalid() -> Self {
        Self::Invalid(ErrorBody {
            error: "invalid authentication request",
        })
    }
}

impl From<jsonwebtoken::errors::Error> for AuthError {
    fn from(_: jsonwebtoken::errors::Error) -> Self {
        tracing::error!("session token signing failed");
        Self::Internal(ErrorBody {
            error: "session unavailable",
        })
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::Invalid(body) => (StatusCode::BAD_REQUEST, body),
            Self::Unauthorized(body) => (StatusCode::UNAUTHORIZED, body),
            Self::Forbidden(body) => (StatusCode::FORBIDDEN, body),
            Self::Internal(body) => (StatusCode::INTERNAL_SERVER_ERROR, body),
        };

        (status, Json(body)).into_response()
    }
}

pub struct AuthRedirect(pub String);

impl<'r> ToResponse<'r> for AuthRedirect {
    fn response() -> (
        &'r str,
        utoipa::openapi::RefOr<utoipa::openapi::response::Response>,
    ) {
        (
            "AuthRedirect",
            utoipa::openapi::response::ResponseBuilder::new()
                .description("Continue the authentication flow")
                .header(
                    "Location",
                    utoipa::openapi::Header::new(utoipa::openapi::Object::with_type(
                        utoipa::openapi::Type::String,
                    )),
                )
                .build()
                .into(),
        )
    }
}

#[derive(IntoResponses)]
pub enum FlowResponse {
    #[response(status = 303)]
    Redirect(#[to_response] AuthRedirect),
}

impl FlowResponse {
    pub fn redirect(location: impl Into<String>) -> Self {
        Self::Redirect(AuthRedirect(location.into()))
    }
}

impl IntoResponse for FlowResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Redirect(AuthRedirect(location)) => Redirect::to(&location).into_response(),
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionBody {
    pub user: Option<User>,
    pub csrf_token: Option<String>,
}

#[derive(IntoResponses)]
pub enum SessionResponse {
    #[response(
        status = 200,
        description = "Current session; user is null when signed out"
    )]
    Success(SessionBody),
}

impl IntoResponse for SessionResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(body) => Json(body).into_response(),
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogoutBody {
    pub redirect_url: String,
}

#[derive(IntoResponses)]
pub enum LogoutResponse {
    #[response(
        status = 200,
        description = "Session revoked; continue to Azure sign-out"
    )]
    Success(LogoutBody),
}

impl IntoResponse for LogoutResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(body) => Json(body).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn auth_errors_use_safe_json_and_correct_http_statuses() {
        let internal = AuthError::from(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken,
        ));

        for (error, status, message) in [
            (
                AuthError::invalid(),
                StatusCode::BAD_REQUEST,
                "invalid authentication request",
            ),
            (
                AuthError::unauthorized(),
                StatusCode::UNAUTHORIZED,
                "authentication required",
            ),
            (
                AuthError::forbidden(),
                StatusCode::FORBIDDEN,
                "access denied",
            ),
            (
                internal,
                StatusCode::INTERNAL_SERVER_ERROR,
                "session unavailable",
            ),
        ] {
            let response = error.into_response();
            assert_eq!(response.status(), status);
            assert_eq!(response.headers()["content-type"], "application/json");

            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                serde_json::json!({"error": message})
            );
        }
    }
}
