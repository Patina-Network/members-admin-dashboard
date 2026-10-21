pub mod openapi;
pub mod state;

mod health;
mod metrics;
mod spa;
mod version;

use std::any::Any;

use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use tower_http::catch_panic::CatchPanicLayer;

use crate::http::state::AppState;

/// JSON body of every `/api` error response; the frontend reads `error`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorBody {
    pub error: &'static str,
}

pub fn app(state: AppState) -> Router {
    let api = Router::new().fallback(not_found);
    Router::new()
        .route("/livez", get(health::live))
        .route("/readyz", get(health::ready))
        .route("/metrics", get(metrics::metrics))
        .route("/version", get(version::version))
        .route("/api/openapi.json", get(openapi::document))
        .nest("/api", api)
        .fallback(spa::spa)
        .layer(CatchPanicLayer::custom(panic_response))
        .with_state(state)
}

/// A panicking handler answers 500 with the usual `ErrorBody` instead of dropping the connection.
fn panic_response(panic: Box<dyn Any + Send + 'static>) -> Response {
    let detail = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload");
    tracing::error!(detail, "handler panicked");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody {
            error: "internal server error",
        }),
    )
        .into_response()
}

/// Unknown `/api` path, for any method.
pub struct NotFound;

impl IntoResponse for NotFound {
    fn into_response(self) -> Response {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorBody { error: "not found" }),
        )
            .into_response()
    }
}

async fn not_found() -> NotFound {
    NotFound
}
