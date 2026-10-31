use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use utoipa::IntoResponses;

use crate::http::state::AppState;

#[derive(IntoResponses)]
pub enum VersionResponse {
    #[response(
        status = 200,
        description = "Service version",
        content_type = "text/plain"
    )]
    Success(String),
}

impl IntoResponse for VersionResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Success(version) => version.into_response(),
        }
    }
}

#[utoipa::path(get, path = "/version", responses(VersionResponse))]
pub async fn version(State(state): State<AppState>) -> VersionResponse {
    VersionResponse::Success(state.config.version.clone().unwrap_or_else(|| "N/A".into()))
}
