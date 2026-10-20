use anyhow::Context;
use hello_world_client_rust::{
    config::Config,
    http::{app, state::AppState},
};
use hello_world_grpc_service::helloworld::greeter_service_client::GreeterServiceClient;
use metrics_exporter_prometheus::PrometheusBuilder;

fn init_tracing() {
    let use_json = std::env::var("ENVIRONMENT")
        .map(|v| v.eq_ignore_ascii_case("production") || v.eq_ignore_ascii_case("staging"))
        .unwrap_or(false);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if use_json {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .pretty()
            .with_env_filter(env_filter)
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .context("failed to install Prometheus recorder")?;
    let sys_collector = metrics_process::Collector::default();
    sys_collector.describe();

    let config = Config::from_env()?;
    config.log();
    let channel = config.grpc_endpoint()?.connect_lazy();
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.http_port)).await?;
    let state = AppState {
        client: GreeterServiceClient::new(channel),
        prometheus_handle,
        sys_collector,
        config: config.clone(),
    };
    tracing::info!(address=%listener.local_addr()?,"HTTP server started");
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler")
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    tokio::select! {_=ctrl_c=>{},_=terminate=>{}}
}
