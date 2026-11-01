use metrics_exporter_prometheus::PrometheusHandle;
use metrics_process::Collector;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub prometheus_handle: PrometheusHandle,
    pub sys_collector: Collector,
    pub config: Config,
}

#[cfg(test)]
pub(crate) fn test_state() -> AppState {
    let auth = crate::config::AuthConfig {
        tenant_id: "11111111-1111-1111-1111-111111111111".into(),
        client_id: "22222222-2222-2222-2222-222222222222".into(),
        client_secret: openidconnect::ClientSecret::new("test-secret".into()),
        origin: "http://localhost:8080".parse().unwrap(),
        admin_emails: vec!["admin@example.com".into()],
        session_keys: crate::auth::session::test_keys(),
        session_ttl: time::Duration::hours(2),
        login_ttl: time::Duration::minutes(10),
        http_timeout: std::time::Duration::from_secs(10),
    };

    AppState {
        prometheus_handle: metrics_exporter_prometheus::PrometheusBuilder::new()
            .build_recorder()
            .handle(),
        sys_collector: Collector::default(),
        config: Config {
            members_admin_service_grpc_host: "localhost:50051".into(),
            members_admin_service_grpc_tls: false,
            members_admin_service_grpc_timeout: std::time::Duration::from_secs(3),
            http_port: 8080,
            enable_documentation: true,
            version: None,
            environment: None,
            log_filter: "info".into(),
            auth,
        },
    }
}
