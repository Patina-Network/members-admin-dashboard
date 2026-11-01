use std::sync::Arc;

use anyhow::Context;
use members_admin_dashboard::{
    auth::provider::AzureProvider,
    config::Config,
    http::{app, auth::AuthApi, members::MembersApi, state::AppState, suggestions::SuggestionsApi},
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

    let auth = AuthApi::new(Arc::new(AzureProvider::new(config.auth.clone())?));

    let channel = config.grpc_endpoint()?.connect_lazy();
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.http_port)).await?;

    let members_grpc_client = MembersServiceClient::new(channel.clone());
    let members_admin_grpc_client = MembersAdminServiceClient::new(channel);
    let timeout = config.members_admin_service_grpc_timeout;

    let state = AppState {
        prometheus_handle,
        sys_collector,
        config: config.clone(),
    };

    tracing::info!(address=%listener.local_addr()?, "HTTP server started");

    let router = app(
        state,
        auth,
        MembersApi::new(
            members_grpc_client.clone(),
            members_admin_grpc_client,
            timeout,
        ),
        SuggestionsApi::new(members_grpc_client, timeout),
    );

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
