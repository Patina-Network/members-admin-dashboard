use std::sync::Arc;

use anyhow::Context;
use members_admin_dashboard::{
    auth::{provider::AzureProvider, session::with_sessions},
    config::Config,
    http::{app, state::AppState},
};
use members_grpc_service::members::{
    admin::members_admin_service_client::MembersAdminServiceClient,
    members_service_client::MembersServiceClient,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use tracing_subscriber::EnvFilter;

fn init_tracing(config: &Config) {
    let env_filter =
        EnvFilter::try_new(&config.log_filter).unwrap_or_else(|_| EnvFilter::new("info"));

    if config.deployed() {
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
    let config = Config::from_env()?;

    init_tracing(&config);
    config.log();

    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .context("failed to install Prometheus recorder")?;
    let sys_collector = metrics_process::Collector::default();
    sys_collector.describe();

    let auth = Arc::new(AzureProvider::new(config.auth.clone())?);

    let channel = config.grpc_endpoint()?.connect_lazy();
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.http_port)).await?;

    let state = AppState {
        client: MembersAdminServiceClient::new(channel.clone()),
        members_client: MembersServiceClient::new(channel),
        prometheus_handle,
        sys_collector,
        config: config.clone(),
        auth,
    };

    tracing::info!(address=%listener.local_addr()?, "HTTP server started");

    let router = with_sessions(app(state), &config.auth);

    axum::serve(listener, router)
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
