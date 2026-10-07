use actix_web::cookie::Key;
use std::time::Duration;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ConfigError {
    #[error("{name} must be a number, got {value:?}")]
    NotANumber { name: &'static str, value: String },
    #[error("SECRET_KEY must be at least 64 characters, got {len}")]
    SecretTooShort { len: usize },
    #[error("SECRET_KEY is required; generate one with `openssl rand -hex 32`")]
    SecretRequired,
    #[error("MASTER_KEY must start with `sk-` and be at least 20 characters")]
    BadMasterKey,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// `None` listens on every interface, IPv6 and IPv4.
    pub host: Option<String>,
    pub port: u16,
    pub database_url: String,
    pub max_connections: u32,
    /// Behind a TLS-terminating proxy: secure cookies, and a fixed secret.
    pub ssl_proxy: bool,
    /// Signs the session cookie and encrypts provider API keys at rest.
    /// Required even in development: unlike Rustrak, a random per-process
    /// secret would not just log people out on restart, it would make every
    /// stored provider key unreadable.
    pub secret_key: String,
    pub dashboard_dir: String,
    pub dashboard_enabled: bool,
    /// How long the gateway waits on a provider before giving up.
    pub upstream_timeout: Duration,
    /// The master key (`MASTER_KEY`, or `LITELLM_MASTER_KEY`): a bearer
    /// token for the management API that acts as an admin. Optional.
    pub api_master_key: Option<String>,
}

impl Config {
    pub const MIN_SECRET_LEN: usize = 64;

    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads configuration through `lookup`, so tests never touch the process
    /// environment.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let var = |name: &str, default: &str| lookup(name).unwrap_or_else(|| default.to_string());
        let ssl_proxy = var("SSL_PROXY", "false") == "true";
        let secret_key = lookup("SECRET_KEY")
            .filter(|s| !s.is_empty())
            .ok_or(ConfigError::SecretRequired)?;
        if secret_key.len() < Self::MIN_SECRET_LEN {
            return Err(ConfigError::SecretTooShort {
                len: secret_key.len(),
            });
        }

        let api_master_key = lookup("MASTER_KEY")
            .or_else(|| lookup("LITELLM_MASTER_KEY"))
            .filter(|k| !k.is_empty());
        if api_master_key
            .as_deref()
            .is_some_and(|k| !k.starts_with("sk-") || k.len() < 20)
        {
            return Err(ConfigError::BadMasterKey);
        }

        Ok(Self {
            api_master_key,
            host: lookup("HOST").filter(|h| !h.trim().is_empty()),
            port: number(&lookup, "PORT", 4000)?,
            database_url: var("DATABASE_URL", default_database_url()),
            max_connections: number(&lookup, "DATABASE_MAX_CONNECTIONS", 10)?,
            ssl_proxy,
            secret_key,
            dashboard_dir: var("LLMTRACK_DASHBOARD_DIR", "./static"),
            dashboard_enabled: var("LLMTRACK_DASHBOARD", "on") != "off",
            upstream_timeout: Duration::from_secs(number(&lookup, "UPSTREAM_TIMEOUT_SECS", 600)?),
        })
    }

    /// The cookie key derived from `SECRET_KEY`. Its 64-byte `master()` also
    /// seeds [`crate::crypto::SecretBox`].
    pub fn master_key(&self) -> Key {
        Key::from(self.secret_key.as_bytes())
    }
}

fn number<T: std::str::FromStr>(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &'static str,
    default: T,
) -> Result<T, ConfigError> {
    match lookup(name) {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| ConfigError::NotANumber { name, value }),
    }
}

fn default_database_url() -> &'static str {
    if cfg!(feature = "postgres") {
        "postgres://llmtrack:llmtrack@localhost:5432/llmtrack"
    } else {
        "sqlite:///data/llmtrack.db"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn config(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let mut map: HashMap<String, String> =
            HashMap::from([("SECRET_KEY".into(), SECRET.into())]);
        map.extend(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        Config::from_lookup(|name| map.get(name).cloned())
    }

    #[test]
    fn defaults_listen_on_port_4000() {
        let config = config(&[]).unwrap();
        assert_eq!(config.port, 4000);
        assert!(config.dashboard_enabled);
        assert_eq!(config.upstream_timeout, Duration::from_secs(600));
    }

    #[test]
    fn without_host_every_interface_is_served() {
        assert_eq!(config(&[]).unwrap().host, None);
        assert_eq!(
            config(&[("HOST", "127.0.0.1")]).unwrap().host.as_deref(),
            Some("127.0.0.1")
        );
    }

    #[test]
    fn a_short_secret_is_refused() {
        assert_eq!(
            config(&[("SECRET_KEY", "short")]).unwrap_err(),
            ConfigError::SecretTooShort { len: 5 }
        );
    }

    #[test]
    fn the_master_key_is_optional_and_starts_with_sk() {
        assert_eq!(config(&[]).unwrap().api_master_key, None);
        let key = "sk-master-0123456789abc";
        assert_eq!(
            config(&[("LITELLM_MASTER_KEY", key)])
                .unwrap()
                .api_master_key
                .as_deref(),
            Some(key)
        );
        assert_eq!(
            config(&[("MASTER_KEY", "letmein")]).unwrap_err(),
            ConfigError::BadMasterKey
        );
    }

    #[test]
    fn a_missing_secret_is_refused() {
        assert_eq!(
            config(&[("SECRET_KEY", "")]).unwrap_err(),
            ConfigError::SecretRequired
        );
    }

    #[test]
    fn a_non_numeric_port_names_the_variable() {
        assert_eq!(
            config(&[("PORT", "http")]).unwrap_err(),
            ConfigError::NotANumber {
                name: "PORT",
                value: "http".into()
            }
        );
    }

    #[test]
    fn the_same_secret_gives_the_same_key() {
        let secret = "a".repeat(64);
        let a = config(&[("SECRET_KEY", &secret)]).unwrap().master_key();
        let b = config(&[("SECRET_KEY", &secret)]).unwrap().master_key();
        assert_eq!(a.master(), b.master());
    }
}
