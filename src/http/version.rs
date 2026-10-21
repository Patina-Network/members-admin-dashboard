use axum::extract::State;

use crate::http::state::AppState;

#[utoipa::path(get, path = "/version", responses((status = 200, body = String, content_type = "text/plain")))]
pub async fn version(State(state): State<AppState>) -> String {
    state.config.version.clone().unwrap_or("N/A".to_string())
}
