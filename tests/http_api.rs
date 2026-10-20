use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use hello_world_client_rust::{
    config::Config,
    http::{app, state::AppState},
};
use hello_world_grpc_service::helloworld::{
    self as pb,
    greeter_service_client::GreeterServiceClient,
    greeter_service_server::{GreeterService, GreeterServiceServer},
};
use http_body_util::BodyExt;
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Response, Status};
use tower::ServiceExt;

#[derive(Default)]
struct Calls {
    sent: Option<pb::SayGreetingRequest>,
    filter: Option<pb::GetGreetingsByNameRequest>,
}
#[derive(Clone, Default)]
struct Mock {
    calls: Arc<Mutex<Calls>>,
}
#[tonic::async_trait]
impl GreeterService for Mock {
    async fn echo_hello(
        &self,
        r: tonic::Request<pb::EchoHelloRequest>,
    ) -> Result<Response<pb::EchoHelloResponse>, Status> {
        let name = r.into_inner().name;
        match name.as_str() {
            "unavailable" => return Err(Status::unavailable("private infrastructure details")),
            "invalid" => return Err(Status::invalid_argument("bad name")),
            "slow" => tokio::time::sleep(Duration::from_secs(1)).await,
            _ => {}
        }
        Ok(Response::new(pb::EchoHelloResponse {
            response: format!("hello {name}"),
        }))
    }
    async fn say_greeting(
        &self,
        r: tonic::Request<pb::SayGreetingRequest>,
    ) -> Result<Response<pb::SayGreetingResponse>, Status> {
        self.calls.lock().unwrap().sent = Some(r.into_inner());
        Ok(Response::new(pb::SayGreetingResponse {}))
    }
    async fn get_greetings_by_name(
        &self,
        r: tonic::Request<pb::GetGreetingsByNameRequest>,
    ) -> Result<Response<pb::GreetingsResponse>, Status> {
        let r = r.into_inner();
        if r.recipient_name.as_deref() == Some("missing") {
            return Err(Status::not_found("missing"));
        }
        self.calls.lock().unwrap().filter = Some(r);
        Ok(Response::new(pb::GreetingsResponse {
            replies: vec![pb::GreetingResponse {
                id: u32::MAX,
                message: "hi".into(),
                sender_name: "Ada".into(),
                recipient_name: "Lin".into(),
                received_at: Some(prost_types::Timestamp {
                    seconds: 0,
                    nanos: 0,
                }),
            }],
        }))
    }
}
struct Fixture {
    app: Router,
    calls: Arc<Mutex<Calls>>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn setup() -> Fixture {
    let mock = Mock::default();
    let calls = mock.calls.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(GreeterServiceServer::new(mock))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let client = GreeterServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();
    let timeout = Duration::from_millis(100);
    Fixture {
        app: app(AppState {
            client,
            prometheus_handle: PrometheusBuilder::new().build_recorder().handle(),
            sys_collector: metrics_process::Collector::default(),
            config: Config {
                hello_world_service_grpc_host: addr.to_string(),
                hello_world_service_grpc_tls: false,
                hello_world_service_grpc_timeout: timeout,
                http_port: 0,
                version: None,
                hello_world_client_urls: vec!["https://go.example.com".into()],
            },
        }),
        calls,
        server,
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
async fn all_rpcs_use_generated_client() {
    let f = setup().await;
    assert_eq!(
        call(&f.app, "GET", "/api/echo?name=Ada", "").await,
        (StatusCode::OK, json!({"response":"hello Ada"}))
    );
    assert_eq!(
        call(&f.app, "GET", "/urls", "").await,
        (StatusCode::OK, json!(["https://go.example.com"]))
    );
    assert_eq!(
        call(
            &f.app,
            "POST",
            "/api/greetings",
            r#"{"senderName":"Ada","recipientName":"Lin","greeting":"hi"}"#
        )
        .await
        .0,
        StatusCode::OK
    );
    {
        let calls = f.calls.lock().unwrap();
        let r = calls.sent.as_ref().unwrap();
        assert_eq!(
            (&*r.sender_name, &*r.recipient_name, &*r.greeting),
            ("Ada", "Lin", "hi")
        );
    }
    for (path, name) in [
        ("/api/greetings", None),
        ("/api/greetings?recipientName=", Some("")),
        ("/api/greetings?recipientName=Lin", Some("Lin")),
    ] {
        let (status, body) = call(&f.app, "GET", path, "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["replies"][0]["id"], json!(u32::MAX));
        assert_eq!(body["replies"][0]["receivedAt"], "1970-01-01T00:00:00Z");
        assert_eq!(
            f.calls
                .lock()
                .unwrap()
                .filter
                .as_ref()
                .unwrap()
                .recipient_name
                .as_deref(),
            name
        );
    }
}
#[tokio::test]
async fn failures_validation_and_deadlines() {
    let f = setup().await;
    for (method, path, body, code) in [
        ("GET", "/api/echo?name=invalid", "", 400),
        ("GET", "/api/echo?name=unavailable", "", 503),
        ("GET", "/api/echo?name=slow", "", 504),
        ("GET", "/api/greetings?recipientName=missing", "", 404),
        ("POST", "/api/greetings", "{", 400),
        ("POST", "/api/greetings", "{}", 400),
        (
            "POST",
            "/api/greetings",
            r#"{"senderName":"A","recipientName":"B","greeting":"hi","unknown":1}"#,
            400,
        ),
        (
            "POST",
            "/api/greetings",
            r#"{"senderName":"A","recipientName":"B","greeting":"hi"} {}"#,
            400,
        ),
        ("GET", "/api/nope", "", 404),
        ("POST", "/api/nope/deeper", "", 404),
    ] {
        let (status, body) = call(&f.app, method, path, body).await;
        assert_eq!(status.as_u16(), code, "{path}: {body}");
        assert!(!body.to_string().contains("private infrastructure"));
    }
    for path in ["/livez", "/readyz"] {
        assert_eq!(
            call_raw(&f.app, "GET", path, "").await,
            (StatusCode::OK, b"ok".to_vec()),
            "{path}"
        );
    }
}
