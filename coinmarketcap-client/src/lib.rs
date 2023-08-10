use anyhow::Result;
use http_client::settings::HttpClientSettings;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;

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

impl Default for CoinmarketcapClient {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

// core functionality
impl CoinmarketcapClient {
    fn build_cryptocurrency_info_url(&self, address: String) -> String {
        format!("{url}/{CRYPTOCURRENCY_INFO}?address={address}", url = self.base_url)
    }

    async fn request<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?;

        Ok(response.json().await?)
    }
}

// Pub api
impl CoinmarketcapClient {
    pub fn new(settings: HttpClientSettings) -> Self {
        let client = (&settings).into();
        let (base_url, api_key) = if settings.is_sandbox {
            (
                SANDBOX_URL.into(),
                settings.api_key.unwrap_or_else(|| SANDBOX_API_KEY.into()),
            )
        } else {
            (URL.into(), settings.api_key.expect("Missing CMC API key"))
        };

        Self {
            base_url,
            client,
            api_key,
        }
    }

    pub async fn cryptocurrency_info(&self, address: String) -> Result<Value> {
        self.request(self.build_cryptocurrency_info_url(address).as_str()).await
    }
}
