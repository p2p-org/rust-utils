use std::time::Duration;

use gcloud_env::GCloudRunEnv;
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSeconds};

lazy_static! {
    pub static ref GCLOUD_ENV: Option<GCloudRunEnv> = GCloudRunEnv::from_env().ok();
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct RpcClientSettings {
    pub address: String,
    #[serde(
        rename = "reconnect_timeout_ms",
        default = "RpcClientSettings::default_reconnect_timeout"
    )]
    #[serde_as(as = "DurationMilliSeconds")]
    pub reconnect_timeout: Duration,
    #[serde(default)]
    pub max_retries: Option<usize>,
}

impl RpcClientSettings {
    fn default_reconnect_timeout() -> Duration {
        Duration::from_secs(1)
    }
}

pub fn default_bind_address() -> String {
    default_bind_address_with_port(8000)
}

pub fn default_bind_address_with_port(port: u16) -> String {
    if let Some(gcloud) = &*GCLOUD_ENV {
        format!("0.0.0.0:{}", gcloud.port)
    } else {
        format!("0.0.0.0:{port}")
    }
}
