use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use members_admin_dashboard::{
    config::Config,
    http::{app, state::AppState},
};
use members_grpc_service::members::admin::members_admin_service_client::MembersAdminServiceClient;
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::{Value, json};
use std::time::Duration;
use tonic::transport::Endpoint;
use tower::ServiceExt;

struct Fixture {
    app: Router,
}

async fn setup() -> Fixture {
    let timeout = Duration::from_millis(100);
    let channel = Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
    Fixture {
        app: app(AppState {
            client: MembersAdminServiceClient::new(channel),
            prometheus_handle: PrometheusBuilder::new().build_recorder().handle(),
            sys_collector: metrics_process::Collector::default(),
            config: Config {
                members_admin_service_grpc_host: "127.0.0.1:1".into(),
                members_admin_service_grpc_tls: false,
                members_admin_service_grpc_timeout: timeout,
                http_port: 0,
                version: Some("test".into()),
            },
        }),
    }
}

async fn call_raw(app: &Router, method: &str, path: &str, body: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

async fn call(app: &Router, method: &str, path: &str, body: &str) -> (StatusCode, Value) {
    let (status, bytes) = call_raw(app, method, path, body).await;
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn health_version_and_unknown_api() {
    let f = setup().await;
    for path in ["/livez", "/readyz"] {
        assert_eq!(
            call_raw(&f.app, "GET", path, "").await,
            (StatusCode::OK, b"ok".to_vec()),
            "{path}"
        );
    }
    assert_eq!(
        call_raw(&f.app, "GET", "/version", "").await,
        (StatusCode::OK, b"test".to_vec())
    );
    assert_eq!(
        call(&f.app, "GET", "/api/nope", "").await,
        (StatusCode::NOT_FOUND, json!({"error": "not found"}))
    );
    let (status, _) = call_raw(&f.app, "GET", "/", "").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn openapi_contract_and_spa_routes_are_available_without_upstream() {
    let f = setup().await;
    let (status, schema) = call(&f.app, "GET", "/api/openapi.json", "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(schema["paths"]["/version"]["get"].is_object());
    assert_eq!(
        schema["components"]["schemas"]["ErrorBody"]["properties"]["error"]["type"],
        "string"
    );
    let (_, root) = call_raw(&f.app, "GET", "/", "").await;
    for path in ["/login", "/database"] {
        let (status, body) = call_raw(&f.app, "GET", path, "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, root);
    }
}

#[tokio::test]
async fn missing_assets_do_not_return_the_spa_html() {
    let f = setup().await;
    for path in ["/assets/missing.js", "/missing.png"] {
        let (status, _) = call_raw(&f.app, "GET", path, "").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
