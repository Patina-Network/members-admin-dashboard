use anyhow::Context;
use members_admin_dashboard::{
    auth::{config::AuthConfig, provider::AzureProvider, session::with_sessions},
    config::Config,
    http::{app, state::AppState},
};
use members_grpc_service::members::{
    admin::members_admin_service_client::MembersAdminServiceClient,
    members_service_client::MembersServiceClient,
};
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
    let auth_config = AuthConfig::from_env()?;
    let auth = auth_config
        .clone()
        .map(AzureProvider::new)
        .transpose()?
        .map(std::sync::Arc::new);
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
    let router = with_sessions(app(state), auth_config.as_ref());
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
