use axum::Json;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(super::version::version), components(schemas(super::ErrorBody)))]
pub struct ApiDoc;

pub async fn document() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
