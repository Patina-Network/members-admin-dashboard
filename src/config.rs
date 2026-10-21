use std::{env, time::Duration};

use tonic::transport::{ClientTlsConfig, Endpoint};
use tracing::info;

#[derive(Debug, Clone)]
pub struct Config {
    pub members_admin_service_grpc_host: String,
    pub members_admin_service_grpc_tls: bool,
    pub members_admin_service_grpc_timeout: Duration,
    pub http_port: u16,
    pub version: Option<String>,
}

impl Config {
    pub fn log(&self) {
        info!(
            members_admin_service_grpc_host = %self.members_admin_service_grpc_host,
            members_admin_service_grpc_tls = self.members_admin_service_grpc_tls,
            members_admin_service_grpc_timeout = ?self.members_admin_service_grpc_timeout,
            http_port = self.http_port,
            version = %self.version.clone().unwrap_or_else(|| "N/A".into()),
            "loaded config"
        );
    }

    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::from_vars(|key| env::var(key).ok()))
    }

    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            members_admin_service_grpc_host: var("MEMBERS_ADMIN_SERVICE_GRPC_HOST")
                .unwrap_or_else(|| "members-admin-grpc-service:50051".into()),
            members_admin_service_grpc_tls: var("MEMBERS_ADMIN_SERVICE_GRPC_TLS")
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            members_admin_service_grpc_timeout: Duration::from_millis(
                var("MEMBERS_ADMIN_SERVICE_GRPC_TIMEOUT_MS")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3000),
            ),
            http_port: var("HTTP_PORT")
                .and_then(|s| s.parse().ok())
                .filter(|port| (1..=65535).contains(port))
                .unwrap_or(8080),
            version: var("VERSION"),
        }
    }

    pub fn grpc_endpoint(&self) -> anyhow::Result<Endpoint> {
        let scheme = if self.members_admin_service_grpc_tls {
            "https"
        } else {
            "http"
        };
        let endpoint = Endpoint::from_shared(format!(
            "{scheme}://{}",
            self.members_admin_service_grpc_host
        ))?
        .connect_timeout(self.members_admin_service_grpc_timeout)
        .timeout(self.members_admin_service_grpc_timeout);
        if !self.members_admin_service_grpc_tls {
            return Ok(endpoint);
        }
        Ok(endpoint.tls_config(ClientTlsConfig::new().with_native_roots())?)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use super::Config;

    fn config_from(vars: &[(&str, &str)]) -> Config {
        let vars: HashMap<&str, &str> = vars.iter().copied().collect();
        Config::from_vars(|key| vars.get(key).map(|v| (*v).to_owned()))
    }

    #[test]
    fn uses_defaults_when_unset() {
        let config = config_from(&[]);

        assert_eq!(
            config.members_admin_service_grpc_host,
            "members-admin-grpc-service:50051"
        );
        assert!(!config.members_admin_service_grpc_tls);
        assert_eq!(
            config.members_admin_service_grpc_timeout,
            Duration::from_millis(3000)
        );
        assert_eq!(config.http_port, 8080);
    }

    #[test]
    fn reads_set_values() {
        let config = config_from(&[
            ("MEMBERS_ADMIN_SERVICE_GRPC_HOST", "grpc.example.com:443"),
            ("MEMBERS_ADMIN_SERVICE_GRPC_TLS", "true"),
            ("MEMBERS_ADMIN_SERVICE_GRPC_TIMEOUT_MS", "30000"),
            ("HTTP_PORT", "9000"),
        ]);

        assert_eq!(
            config.members_admin_service_grpc_host,
            "grpc.example.com:443"
        );
        assert!(config.members_admin_service_grpc_tls);
        assert_eq!(
            config.members_admin_service_grpc_timeout,
            Duration::from_millis(30000)
        );
        assert_eq!(config.http_port, 9000);
    }

    #[test]
    fn falls_back_to_defaults_on_invalid_values() {
        for (key, value) in [
            ("MEMBERS_ADMIN_SERVICE_GRPC_TLS", "yes"),
            ("MEMBERS_ADMIN_SERVICE_GRPC_TIMEOUT_MS", "fast"),
            ("HTTP_PORT", "70000"),
            ("HTTP_PORT", "http"),
        ] {
            let config = config_from(&[(key, value)]);

            assert!(!config.members_admin_service_grpc_tls, "{key}={value}");
            assert_eq!(
                config.members_admin_service_grpc_timeout,
                Duration::from_millis(3000),
                "{key}={value}"
            );
            assert_eq!(config.http_port, 8080, "{key}={value}");
        }
    }

    #[test]
    fn plaintext_endpoint_uses_http_scheme() {
        let endpoint = config_from(&[("MEMBERS_ADMIN_SERVICE_GRPC_HOST", "localhost:50051")])
            .grpc_endpoint()
            .unwrap();

        assert_eq!(endpoint.uri().to_string(), "http://localhost:50051/");
    }
}
