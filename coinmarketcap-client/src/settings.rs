use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::time::Duration;

#[serde_as]
#[derive(Deserialize, Eq, PartialEq, Debug)]
pub struct HttpClientSettings {
    #[serde(rename = "tcp_keepalive_sec", default = "HttpClientSettings::default_tcp_keepalive")]
    #[serde_as(as = "DurationSeconds")]
    pub tcp_keepalive: Duration,
    #[serde(
        rename = "pool_idle_timeout_sec",
        default = "HttpClientSettings::default_pool_idle_timeout"
    )]
    #[serde_as(as = "DurationSeconds")]
    pub pool_idle_timeout: Duration,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub is_sandbox: bool,
    #[serde(default = "HttpClientSettings::default_enabled")]
    pub enabled: bool,
    #[serde(default = "HttpClientSettings::default_history_chunk_size")]
    pub history_chunk_size: usize,
}

impl From<&HttpClientSettings> for reqwest::Client {
    fn from(settings: &HttpClientSettings) -> Self {
        reqwest::ClientBuilder::new()
            .tcp_keepalive(Some(settings.tcp_keepalive))
            .pool_idle_timeout(Some(settings.pool_idle_timeout))
            .build()
            .expect("Client must be built")
    }
}

impl HttpClientSettings {
    fn default_tcp_keepalive() -> Duration {
        Duration::from_secs(20)
    }

    fn default_pool_idle_timeout() -> Duration {
        Duration::from_secs(20)
    }

    fn default_enabled() -> bool {
        false
    }

    fn default_history_chunk_size() -> usize {
        10
    }

    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }
}

impl Default for HttpClientSettings {
    fn default() -> Self {
        Self {
            tcp_keepalive: Duration::from_secs(20),
            pool_idle_timeout: Duration::from_secs(20),
            api_key: None,
            is_sandbox: false,
            enabled: Self::default_enabled(),
            history_chunk_size: Self::default_history_chunk_size(),
        }
    }
}

impl HttpClientSettings {
    pub fn sandbox() -> Self {
        Self {
            is_sandbox: true,
            ..Default::default()
        }
    }
}
