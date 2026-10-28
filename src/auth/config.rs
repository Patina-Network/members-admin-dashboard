use anyhow::{Context, bail, ensure};
use openidconnect::{ClientSecret, url::Url};

#[derive(Clone)]
pub struct AuthConfig {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: ClientSecret,
    pub origin: Url,
    pub admin_emails: Vec<String>,
}

impl AuthConfig {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        Self::from_vars(|key| std::env::var(key).ok())
    }

    fn from_vars(var: impl Fn(&str) -> Option<String>) -> anyhow::Result<Option<Self>> {
        let required = [
            "AZURE_TENANT_ID",
            "AZURE_CLIENT_ID",
            "AZURE_CLIENT_SECRET",
            "APP_ORIGIN",
        ];
        let values: Vec<_> = required.iter().map(|key| var(key)).collect();
        let deployed = var("ENVIRONMENT").is_some_and(|v| {
            v.eq_ignore_ascii_case("production") || v.eq_ignore_ascii_case("staging")
        });
        if values.iter().all(Option::is_none) && !deployed {
            return Ok(None);
        }
        for (key, value) in required.iter().zip(&values) {
            ensure!(
                value.as_ref().is_some_and(|v| !v.trim().is_empty()),
                "{key} is required for Azure login"
            );
        }
        let mut values = values.into_iter().map(Option::unwrap);
        let tenant_id = values.next().unwrap();
        ensure!(
            is_uuid(&tenant_id),
            "AZURE_TENANT_ID must be a tenant UUID (not common or organizations)"
        );
        let client_id = values.next().unwrap();
        ensure!(
            is_uuid(&client_id),
            "AZURE_CLIENT_ID must be an application UUID"
        );
        let client_secret = ClientSecret::new(values.next().unwrap());
        let origin =
            Url::parse(&values.next().unwrap()).context("APP_ORIGIN must be an absolute URL")?;
        let local = matches!(origin.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if origin.scheme() != "https" && !(origin.scheme() == "http" && local && !deployed) {
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
        let admin_emails = var("AZURE_ADMIN_EMAILS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|email| !email.is_empty())
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();
        for email in &admin_emails {
            ensure!(
                email.split_once('@').is_some_and(|(local, domain)| {
                    !local.is_empty() && !domain.is_empty() && !domain.contains('@')
                }) && !email.chars().any(char::is_whitespace)
                    && !email.contains('*'),
                "AZURE_ADMIN_EMAILS must contain comma-separated email addresses"
            );
        }
        Ok(Some(Self {
            tenant_id,
            client_id,
            client_secret,
            origin,
            admin_emails,
        }))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn configured(key: &str) -> Option<String> {
        Some(
            match key {
                "AZURE_TENANT_ID" => "11111111-1111-1111-1111-111111111111",
                "AZURE_CLIENT_ID" => "22222222-2222-2222-2222-222222222222",
                "AZURE_CLIENT_SECRET" => "test-secret",
                "APP_ORIGIN" => "http://localhost:5173",
                _ => return None,
            }
            .into(),
        )
    }

    #[test]
    fn local_defaults_and_urls() {
        let config = AuthConfig::from_vars(configured).unwrap().unwrap();
        assert!(config.admin_emails.is_empty());
        assert!(!config.secure_cookie());
        assert_eq!(
            config.callback_url(),
            "http://localhost:5173/oauth2/callback"
        );
        assert!(AuthConfig::from_vars(|_| None).unwrap().is_none());
    }

    #[test]
    fn parses_exact_email_allowlist_and_denies_empty_configuration() {
        let config = AuthConfig::from_vars(|key| {
            if key == "AZURE_ADMIN_EMAILS" {
                Some(" Alice@Example.com, bob@example.com, ,".into())
            } else {
                configured(key)
            }
        })
        .unwrap()
        .unwrap();
        assert!(config.allows_email(Some("alice@example.com")));
        assert!(config.allows_email(Some("BOB@EXAMPLE.COM")));
        assert!(!config.allows_email(Some("other@example.com")));
        assert!(!config.allows_email(Some("alice@example.com.attacker.test")));
        assert!(!config.allows_email(None));
        let empty = AuthConfig::from_vars(configured).unwrap().unwrap();
        assert!(!empty.allows_email(Some("alice@example.com")));
    }

    #[test]
    fn rejects_partial_or_unsafe_configuration() {
        for (key, value) in [
            ("AZURE_TENANT_ID", "common"),
            ("AZURE_CLIENT_SECRET", ""),
            ("APP_ORIGIN", "http://example.com"),
            ("APP_ORIGIN", "https://example.com/path"),
            ("APP_ORIGIN", "https://user:password@example.com"),
            ("AZURE_ADMIN_EMAILS", "*@example.com"),
            ("AZURE_ADMIN_EMAILS", "not-an-email"),
            ("ENVIRONMENT", "production"),
        ] {
            assert!(
                AuthConfig::from_vars(|k| if k == key {
                    Some(value.into())
                } else {
                    configured(k)
                })
                .is_err(),
                "{key}"
            );
        }
        assert!(
            AuthConfig::from_vars(|k| if k == "ENVIRONMENT" {
                Some("staging".into())
            } else {
                None
            })
            .is_err()
        );
    }
}
