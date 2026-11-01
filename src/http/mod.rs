pub mod auth;
pub mod openapi;
pub mod state;

mod health;
pub mod members;
mod metrics;
mod spa;
pub mod suggestions;
mod version;

use std::any::Any;

use axum::{
    Json, Router,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use tower_http::catch_panic::{self};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    grpc::{MembersAdminGrpcClient, MembersGrpcClient},
    http::{auth::AuthApi, members::MembersApi, state::AppState, suggestions::SuggestionsApi},
};

/// JSON body of every `/api` error response; the frontend reads `error`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorBody {
    pub error: &'static str,
}

pub fn app<MGC: MembersGrpcClient, MAGC: MembersAdminGrpcClient>(
    state: AppState,
    auth: AuthApi,
    members: MembersApi<MGC, MAGC>,
    suggestions: SuggestionsApi<MGC>,
) -> Router {
    let api = Router::new()
        .merge(auth.clone().router())
        .merge(members.router())
        .merge(suggestions.router())
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
        .merge(auth.oauth_callback_router())
        .nest("/api", api)
        .fallback(spa::spa)
        .layer(middleware::from_fn_with_state(
            state.config.auth.clone(),
            auth::middleware::protect,
        ))
        .layer(catch_panic::CatchPanicLayer::custom(panic_response))
        .with_state(state)
}

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
