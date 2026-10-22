use members_grpc_service::members::admin::members_admin_service_client::MembersAdminServiceClient;
use metrics_exporter_prometheus::PrometheusHandle;
use metrics_process::Collector;
use tonic::transport::Channel;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub client: MembersAdminServiceClient<Channel>,
    pub prometheus_handle: PrometheusHandle,
    pub sys_collector: Collector,
    pub config: Config,
}
