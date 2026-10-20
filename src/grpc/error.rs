use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tonic::{Code, Status};

use crate::http::ErrorBody;

#[derive(Debug)]
pub enum UpstreamError {
    InvalidArgument,
    NotFound,
    AlreadyExists,
    Unauthenticated,
    PermissionDenied,
    ResourceExhausted,
    Unavailable,
    Timeout,
    Failed,
}

impl From<Status> for UpstreamError {
    fn from(status: Status) -> Self {
        tracing::warn!(code = ?status.code(), detail = status.message(), "gRPC request failed");
        match status.code() {
            Code::InvalidArgument => Self::InvalidArgument,
            Code::NotFound => Self::NotFound,
            Code::AlreadyExists => Self::AlreadyExists,
            Code::Unauthenticated => Self::Unauthenticated,
            Code::PermissionDenied => Self::PermissionDenied,
            Code::ResourceExhausted => Self::ResourceExhausted,
            Code::Unavailable => Self::Unavailable,
            Code::DeadlineExceeded | Code::Cancelled => Self::Timeout,
            _ => Self::Failed,
        }
    }
}

impl IntoResponse for UpstreamError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::InvalidArgument => (StatusCode::BAD_REQUEST, "invalid request"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not found"),
            Self::AlreadyExists => (StatusCode::CONFLICT, "already exists"),
            Self::Unauthenticated => (StatusCode::UNAUTHORIZED, "authentication required"),
            Self::PermissionDenied => (StatusCode::FORBIDDEN, "permission denied"),
            Self::ResourceExhausted => (StatusCode::TOO_MANY_REQUESTS, "resource exhausted"),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "service unavailable"),
            Self::Timeout => (StatusCode::GATEWAY_TIMEOUT, "upstream timeout"),
            Self::Failed => (StatusCode::BAD_GATEWAY, "upstream request failed"),
        };
        (status, Json(ErrorBody { error })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};
    use http_body_util::BodyExt;
    use tonic::{Code, Status};

    use super::UpstreamError;

    #[tokio::test]
    async fn grpc_codes_map_to_http_errors_without_leaking_detail() {
        for (code, expected_status, expected_error) in [
            (
                Code::InvalidArgument,
                StatusCode::BAD_REQUEST,
                "invalid request",
            ),
            (Code::NotFound, StatusCode::NOT_FOUND, "not found"),
            (Code::AlreadyExists, StatusCode::CONFLICT, "already exists"),
            (
                Code::Unauthenticated,
                StatusCode::UNAUTHORIZED,
                "authentication required",
            ),
            (
                Code::PermissionDenied,
                StatusCode::FORBIDDEN,
                "permission denied",
            ),
            (
                Code::ResourceExhausted,
                StatusCode::TOO_MANY_REQUESTS,
                "resource exhausted",
            ),
            (
                Code::Unavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "service unavailable",
            ),
            (
                Code::DeadlineExceeded,
                StatusCode::GATEWAY_TIMEOUT,
                "upstream timeout",
            ),
            (
                Code::Cancelled,
                StatusCode::GATEWAY_TIMEOUT,
                "upstream timeout",
            ),
            (
                Code::Internal,
                StatusCode::BAD_GATEWAY,
                "upstream request failed",
            ),
            (
                Code::Unknown,
                StatusCode::BAD_GATEWAY,
                "upstream request failed",
            ),
        ] {
            let response =
                UpstreamError::from(Status::new(code, "private upstream detail")).into_response();
            assert_eq!(response.status(), expected_status, "{code:?}");

            let body = response.into_body().collect().await.unwrap().to_bytes();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                body,
                serde_json::json!({ "error": expected_error }),
                "{code:?}"
            );
        }
    }
}
