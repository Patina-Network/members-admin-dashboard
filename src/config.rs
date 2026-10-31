use std::{env, time::Duration};

use anyhow::{Context, bail, ensure};
use openidconnect::{ClientSecret, url::Url};
use tonic::transport::{ClientTlsConfig, Endpoint};
use tracing::info;

use crate::{auth::session::SessionKeys, duration::DurationString};

#[derive(Debug, Clone)]
pub struct Config {
    pub members_admin_service_grpc_host: String,
    pub members_admin_service_grpc_tls: bool,
    pub members_admin_service_grpc_timeout: Duration,
    pub http_port: u16,
    pub enable_documentation: bool,
    pub version: Option<String>,
    pub environment: Option<String>,
    pub log_filter: String,
    pub auth: AuthConfig,
}

impl Config {
    pub fn log(&self) {
        info!(
            members_admin_service_grpc_host = %self.members_admin_service_grpc_host,
            members_admin_service_grpc_tls = self.members_admin_service_grpc_tls,
            members_admin_service_grpc_timeout = ?self.members_admin_service_grpc_timeout,
            http_port = self.http_port,
            enable_documentation = self.enable_documentation,
            version = %self.version.clone().unwrap_or_else(|| "N/A".into()),
            environment = self.environment.as_deref().unwrap_or("N/A"),
            log_filter = %self.log_filter,
            app_origin = %self.auth.origin,
            session_ttl = %self.auth.session_ttl,
            login_ttl = %self.auth.login_ttl,
            azure_http_timeout = ?self.auth.http_timeout,
            "loaded config"
        );
    }

    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            members_admin_service_grpc_host: env::var("MEMBERS_ADMIN_SERVICE_GRPC_HOST")
                .unwrap_or_else(|_| "members-admin-grpc-service:50051".into()),
            members_admin_service_grpc_tls: env::var("MEMBERS_ADMIN_SERVICE_GRPC_TLS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            members_admin_service_grpc_timeout: Duration::from_millis(
                env::var("MEMBERS_ADMIN_SERVICE_GRPC_TIMEOUT_MS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3000),
            ),
            http_port: env::var("HTTP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|port| (1..=65535).contains(port))
                .unwrap_or(8080),
            enable_documentation: env::var("ENABLE_DOCUMENTATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            version: env::var("VERSION").ok(),
            environment: env::var("ENVIRONMENT").ok(),
            log_filter: env::var("RUST_LOG")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "info".into()),
            auth: AuthConfig::from_env(is_deployed(env::var("ENVIRONMENT").ok().as_deref()))?,
        })
    }

    pub fn deployed(&self) -> bool {
        is_deployed(self.environment.as_deref())
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

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: ClientSecret,
    pub origin: Url,
    pub admin_emails: Vec<String>,
    /// HS256 key for session cookies, from `SESSION_SECRET`.
    pub session_keys: SessionKeys,
    /// Session cookie/JWT lifetime, from `SESSION_TTL`.
    pub session_ttl: time::Duration,
    /// In-flight Azure sign-in lifetime, from `LOGIN_TTL`.
    pub login_ttl: time::Duration,
    pub http_timeout: Duration,
}

impl AuthConfig {
    fn from_env(deployed: bool) -> anyhow::Result<Self> {
        Ok(Self {
            tenant_id: {
                let tenant_id = required("AZURE_TENANT_ID")?;
                ensure!(
                    is_uuid(&tenant_id),
                    "AZURE_TENANT_ID must be a tenant UUID (not common or organizations)"
                );
                tenant_id
            },
            client_id: {
                let client_id = required("AZURE_CLIENT_ID")?;
                ensure!(
                    is_uuid(&client_id),
                    "AZURE_CLIENT_ID must be an application UUID"
                );
                client_id
            },
            client_secret: ClientSecret::new(required("AZURE_CLIENT_SECRET")?),
            origin: {
                let origin = Url::parse(&required("APP_ORIGIN")?)
                    .context("APP_ORIGIN must be an absolute URL")?;
                let local = matches!(origin.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
                if origin.scheme() != "https" && !(origin.scheme() == "http" && local && !deployed)
                {
                    bail!("APP_ORIGIN must use HTTPS, except on localhost during development");
                }
                ensure!(
                    origin.host_str().is_some()
                        && origin.username().is_empty()
                        && origin.password().is_none()
                        && origin.path() == "/"
                        && origin.query().is_none()
                        && origin.fragment().is_none(),
                    "APP_ORIGIN must contain only the scheme, host, and optional port"
                );
                origin
            },
            admin_emails: {
                let admin_emails = env::var("AZURE_ADMIN_EMAILS")
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|email| !email.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();

                for email in &admin_emails {
                    ensure!(
                        email.split_once('@').is_some_and(|(local, domain)| {
                            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
                        }) && !email.chars().any(char::is_whitespace)
                            && !email.contains('*'),
                        "AZURE_ADMIN_EMAILS must contain comma-separated valid email addresses."
                    );
                }
                admin_emails
            },
            session_keys: {
                let secret = required("SESSION_SECRET")?;
                ensure!(
                    secret.len() >= 32,
                    "SESSION_SECRET must be at least 32 bytes; generate one with `openssl rand -base64 48`"
                );
                SessionKeys::new(secret.as_bytes())
            },
            session_ttl: browser_ttl("SESSION_TTL", "2h")?,
            login_ttl: browser_ttl("LOGIN_TTL", "10m")?,
            http_timeout: duration("AZURE_HTTP_TIMEOUT", "10s")?,
        })
    }

    pub fn allows_email(&self, email: Option<&str>) -> bool {
        email.is_some_and(|email| {
            self.admin_emails
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(email))
        })
    }

    pub fn secure_cookie(&self) -> bool {
        self.origin.scheme() == "https"
    }

    pub fn callback_url(&self) -> String {
        format!("{}oauth2/callback", self.origin)
    }

    pub fn logout_url(&self) -> String {
        let mut url = Url::parse(&format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/logout",
            self.tenant_id
        ))
        .unwrap();

        url.query_pairs_mut()
            .append_pair("post_logout_redirect_uri", &format!("{}login", self.origin));

        url.into()
    }
}

fn is_deployed(environment: Option<&str>) -> bool {
    environment
        .is_some_and(|v| v.eq_ignore_ascii_case("production") || v.eq_ignore_ascii_case("staging"))
}

fn required(name: &str) -> anyhow::Result<String> {
    env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .with_context(|| format!("{name} is required"))
}

fn duration(name: &str, default: &str) -> anyhow::Result<Duration> {
    let raw = env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.into());
    let parsed: Duration = raw
        .parse::<DurationString>()
        .with_context(|| format!("{name} must be a duration like `2h`, `90m`, or `1h30m`"))?
        .into();
    ensure!(
        parsed >= Duration::from_secs(1),
        "{name} must be at least 1s"
    );
    Ok(parsed)
}

const MAX_TTL: Duration = Duration::from_secs(400 * 24 * 60 * 60);

fn browser_ttl(name: &str, default: &str) -> anyhow::Result<time::Duration> {
    let ttl = duration(name, default)?;
    ensure!(
        ttl <= MAX_TTL,
        "{name} must be at most 400d; browsers cap cookie lifetimes at 400 days"
    );
    Ok(time::Duration::try_from(ttl)?)
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}
