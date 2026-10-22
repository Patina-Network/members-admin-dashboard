use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(super::version::version), components(schemas(super::ErrorBody)))]
pub struct ApiDoc;
