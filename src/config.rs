use std::{env, time::Duration};

use tonic::transport::{ClientTlsConfig, Endpoint};
use tracing::info;

#[derive(Debug, Clone)]
pub struct Config {
    pub hello_world_service_grpc_host: String,
    pub hello_world_service_grpc_tls: bool,
    pub hello_world_service_grpc_timeout: Duration,
    pub http_port: u16,
    pub version: Option<String>,
    /// Comma-separated `HELLO_WORLD_CLIENT_URLS`, trimmed, empty entries dropped.
    pub hello_world_client_urls: Vec<String>,
}

impl Config {
    pub fn log(&self) {
        info!(
            hello_world_service_grpc_host = %self.hello_world_service_grpc_host,
            hello_world_service_grpc_tls = self.hello_world_service_grpc_tls,
            hello_world_service_grpc_timeout = ?self.hello_world_service_grpc_timeout,
            http_port = self.http_port,
            version = %self.version.clone().unwrap_or_else(||"N/A".into()),
            hello_world_client_urls = ?self.hello_world_client_urls,
            "loaded config"
        );
    }

    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::from_vars(|key| env::var(key).ok()))
    }

    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            hello_world_service_grpc_host: var("HELLO_WORLD_SERVICE_GRPC_HOST")
                .unwrap_or_else(|| "hello-world-grpc-service:50051".into()),
            hello_world_service_grpc_tls: var("HELLO_WORLD_SERVICE_GRPC_TLS")
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            hello_world_service_grpc_timeout: Duration::from_millis(
                var("HELLO_WORLD_SERVICE_GRPC_TIMEOUT_MS")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3000),
            ),
            http_port: var("HTTP_PORT")
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            version: var("VERSION"),
            hello_world_client_urls: var("HELLO_WORLD_CLIENT_URLS")
                .map(|s| {
                    s.split(',')
                        .map(str::trim)
                        .filter(|url| !url.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn grpc_endpoint(&self) -> anyhow::Result<Endpoint> {
        let scheme = if self.hello_world_service_grpc_tls {
            "https"
        } else {
            "http"
        };
        let endpoint =
            Endpoint::from_shared(format!("{scheme}://{}", self.hello_world_service_grpc_host))?
                .connect_timeout(self.hello_world_service_grpc_timeout)
                .timeout(self.hello_world_service_grpc_timeout);
        if !self.hello_world_service_grpc_tls {
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
            config.hello_world_service_grpc_host,
            "hello-world-grpc-service:50051"
        );
        assert!(!config.hello_world_service_grpc_tls);
        assert_eq!(
            config.hello_world_service_grpc_timeout,
            Duration::from_millis(3000)
        );
        assert_eq!(config.http_port, 8080);
        assert!(config.hello_world_client_urls.is_empty());
    }

    #[test]
    fn reads_set_values() {
        let config = config_from(&[
            ("HELLO_WORLD_SERVICE_GRPC_HOST", "grpc.example.com:443"),
            ("HELLO_WORLD_SERVICE_GRPC_TLS", "true"),
            ("HELLO_WORLD_SERVICE_GRPC_TIMEOUT_MS", "30000"),
            ("HTTP_PORT", "9000"),
            (
                "HELLO_WORLD_CLIENT_URLS",
                " https://go.example.com , ,https://java.example.com,",
            ),
        ]);

        assert_eq!(config.hello_world_service_grpc_host, "grpc.example.com:443");
        assert!(config.hello_world_service_grpc_tls);
        assert_eq!(
            config.hello_world_service_grpc_timeout,
            Duration::from_millis(30000)
        );
        assert_eq!(config.http_port, 9000);
        assert_eq!(
            config.hello_world_client_urls,
            ["https://go.example.com", "https://java.example.com"]
        );
    }

    #[test]
    fn falls_back_to_defaults_on_invalid_values() {
        for (key, value) in [
            ("HELLO_WORLD_SERVICE_GRPC_TLS", "yes"),
            ("HELLO_WORLD_SERVICE_GRPC_TIMEOUT_MS", "fast"),
            ("HTTP_PORT", "70000"),
            ("HTTP_PORT", "http"),
        ] {
            let config = config_from(&[(key, value)]);

            assert!(!config.hello_world_service_grpc_tls, "{key}={value}");
            assert_eq!(
                config.hello_world_service_grpc_timeout,
                Duration::from_millis(3000),
                "{key}={value}"
            );
            assert_eq!(config.http_port, 8080, "{key}={value}");
        }
    }

    #[test]
    fn plaintext_endpoint_uses_http_scheme() {
        let endpoint = config_from(&[("HELLO_WORLD_SERVICE_GRPC_HOST", "localhost:50051")])
            .grpc_endpoint()
            .unwrap();

        assert_eq!(endpoint.uri().to_string(), "http://localhost:50051/");
    }
}
