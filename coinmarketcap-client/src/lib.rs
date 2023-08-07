pub mod settings;

use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::settings::HttpClientSettings;

static URL: &str = "https://pro-api.coinmarketcap.com";

static SANDBOX_URL: &str = "https://sandbox-api.coinmarketcap.com";
static SANDBOX_API_KEY: &str = "b54bcf4d-1bca-4e8e-9a24-22ff2c3d462c";

static CRYPTOCURRENCY_INFO: &str = "v2/cryptocurrency/info";

#[derive(Clone)]
pub struct CoinmarketcapClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl CoinmarketcapClient {
    pub fn new(settings: &HttpClientSettings) -> Self {
        let client: Client = settings.into();
        let (base_url, api_key) = if settings.is_sandbox {
            (
                SANDBOX_URL.into(),
                settings.api_key.clone().unwrap_or_else(|| SANDBOX_API_KEY.into()),
            )
        } else {
            (URL.into(), settings.api_key.clone().expect("Missing CMC API key"))
        };

        Self {
            base_url,
            client,
            api_key,
        }
    }

    pub fn build_cryptocurrency_info_url(&self, address: String) -> String {
        format!("{url}/{CRYPTOCURRENCY_INFO}?address={address}", url = self.base_url)
    }

    pub async fn request<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?;

        Ok(response.json().await?)
    }
}
