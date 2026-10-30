pub mod auth;
pub mod openapi;
pub mod state;

mod health;
mod members;
mod metrics;
mod spa;
mod version;

use std::any::Any;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use tower_http::catch_panic::CatchPanicLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::http::state::AppState;

/// JSON body of every `/api` error response; the frontend reads `error`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorBody {
    pub error: &'static str,
}

pub fn app(state: AppState) -> Router {
    let api = Router::new()
        .route("/members", get(members::list).post(members::create))
        .route(
            "/members/bulk",
            post(members::bulk_create).layer(DefaultBodyLimit::max(2 * 1024 * 1024)),
        )
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/session", get(auth::current_session))
        .route("/auth/logout", post(auth::logout))
        .route(
            "/members/{member_id}",
            get(members::details)
                .put(members::update)
                .delete(members::archive),
        )
        .fallback(not_found);
    let mut router = Router::new();
    if state.config.enable_documentation {
        router = router.merge(
            SwaggerUi::new("/api/swagger-ui").url("/api/openapi.json", openapi::ApiDoc::openapi()),
        );
    }
    router
        .route("/livez", get(health::live))
        .route("/readyz", get(health::ready))
        .route("/metrics", get(metrics::metrics))
        .route("/version", get(version::version))
        .route("/oauth2/callback", get(auth::callback))
        .nest("/api", api)
        .fallback(spa::spa)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::guard::protect,
        ))
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
