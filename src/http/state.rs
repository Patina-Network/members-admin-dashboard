use hello_world_grpc_service::helloworld::greeter_service_client::GreeterServiceClient;
use metrics_exporter_prometheus::PrometheusHandle;
use metrics_process::Collector;
use tonic::transport::Channel;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub client: GreeterServiceClient<Channel>,
    pub prometheus_handle: PrometheusHandle,
    pub sys_collector: Collector,
    pub config: Config,
}
