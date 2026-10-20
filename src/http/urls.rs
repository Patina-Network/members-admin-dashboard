use axum::{Json, extract::State};

use crate::http::state::AppState;

/// The other hello world clients, from `HELLO_WORLD_CLIENT_URLS`.
pub async fn urls(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(state.config.hello_world_client_urls.clone())
}
