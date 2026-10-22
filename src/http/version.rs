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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use http_body_util::BodyExt;
    use members_grpc_service::members::admin::members_admin_service_client::MembersAdminServiceClient;
    use metrics_exporter_prometheus::PrometheusBuilder;
    use std::time::Duration;
    use tonic::transport::Endpoint;

    #[tokio::test]
    async fn returns_configured_version_or_fallback() {
        for (configured, expected) in [(Some("test".to_owned()), "test"), (None, "N/A")] {
            let state = AppState {
                client: MembersAdminServiceClient::new(
                    Endpoint::from_static("http://localhost:50051").connect_lazy(),
                ),
                prometheus_handle: PrometheusBuilder::new().build_recorder().handle(),
                sys_collector: metrics_process::Collector::default(),
                config: Config {
                    members_admin_service_grpc_host: "localhost:50051".into(),
                    members_admin_service_grpc_tls: false,
                    members_admin_service_grpc_timeout: Duration::from_secs(3),
                    http_port: 8080,
                    enable_documentation: true,
                    version: configured,
                },
            };
            let response = version(State(state)).await.into_response();
            assert_eq!(response.status(), axum::http::StatusCode::OK);
            assert_eq!(
                response.headers()["content-type"],
                "text/plain; charset=utf-8"
            );
            let body = response.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(body.as_ref(), expected.as_bytes());
        }
    }
}
