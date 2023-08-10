use anyhow::Context;
use http::{
    header::{ETAG, IF_NONE_MATCH},
    HeaderMap, HeaderName, StatusCode,
};
use http_client::settings::HttpClientSettings;
use serde::Deserialize;
use std::collections::HashMap;
use token_address::StoredTokenAddress;
use types::{CoingeckoInfo, CoingeckoInfoWithAddress};

pub mod types;

const PUBLIC_BASE_URL: &str = "https://api.coingecko.com/api/v3";
const PRO_BASE_URL: &str = "https://pro-api.coingecko.com/api/v3";

pub struct CoingeckoClient {
    client: reqwest::Client,
    base_url: String,
}

impl CoingeckoClient {
    pub fn new(settings: HttpClientSettings) -> anyhow::Result<Self> {
        let HttpClientSettings {
            tcp_keepalive,
            pool_idle_timeout,
            api_key,
            ..
        } = settings;

        let base_url = if api_key.is_some() {
            PRO_BASE_URL
        } else {
            PUBLIC_BASE_URL
        };

        let mut builder = reqwest::ClientBuilder::new()
            .tcp_keepalive(Some(tcp_keepalive))
            .pool_idle_timeout(Some(pool_idle_timeout));

        if let Some(api_key) = api_key {
            builder = builder.default_headers(HeaderMap::from_iter([(
                HeaderName::from_static("x-cg-pro-api-key"),
                api_key.try_into()?,
            )]));
        };

        let client = builder.build().context("Unable to build coingecko client")?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }

    pub async fn get_metadata_by_address(
        &self,
        address: &StoredTokenAddress,
    ) -> anyhow::Result<Option<CoingeckoInfoWithAddress>> {
        let response = self
            .client
            .get(format!(
                "{base_url}/coins/{platform}/contract/{address}",
                base_url = self.base_url,
                platform = address.platform(),
            ))
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response.error_for_status()?;

        Ok(Some(response.json::<CoingeckoCoinsResponse>().await?.into()))
    }

    pub async fn get_metadata_by_slug(&self, slug: &str) -> anyhow::Result<Option<CoingeckoInfoWithAddress>> {
        let response = self
            .client
            .get(format!("{base_url}/coins/{slug}", base_url = self.base_url))
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response.error_for_status()?;

        Ok(Some(response.json::<CoingeckoCoinsResponse>().await?.into()))
    }

    pub async fn get_all_metadata(&self, etag: Option<&String>) -> anyhow::Result<Option<CoingeckoCoinsList>> {
        let mut builder = self.client.get(format!(
            "{base_url}/coins/list?include_platform=true",
            base_url = self.base_url
        ));

        if let Some(etag) = etag {
            builder = builder.header(IF_NONE_MATCH, etag);
        }

        let response = builder.send().await?;

        if etag.is_some() && response.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        let response = response.error_for_status()?;
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);
        let coins_list = response
            .json::<Vec<CoingeckoCoinsResponse>>()
            .await?
            .into_iter()
            .map(|v| (v.id.clone(), v.into()))
            .collect();

        Ok(CoingeckoCoinsList { coins_list, etag }.into())
    }
}

#[derive(Debug)]
pub struct CoingeckoCoinsList {
    pub coins_list: HashMap<String, CoingeckoInfoWithAddress>,
    pub etag: Option<String>,
}

#[derive(Deserialize)]
struct CoingeckoCoinsResponse {
    id: String,
    symbol: String,
    name: String,
    platforms: HashMap<String, Option<String>>,
}

impl From<CoingeckoCoinsResponse> for CoingeckoInfoWithAddress {
    fn from(value: CoingeckoCoinsResponse) -> Self {
        Self {
            metadata: CoingeckoInfo {
                coin_id: value.id,
                symbol: value.symbol,
                name: value.name,
            },
            addresses: value.platforms.into_iter().filter_map(|(k, v)| Some((k, v?))).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CoingeckoClient, CoingeckoCoinsList};
    use claims::{assert_none, assert_some};

    #[tokio::test]
    async fn should_cache_coins_list() -> anyhow::Result<()> {
        let client = CoingeckoClient::new(Default::default())?;
        let coins_list = client.get_all_metadata(None).await?;
        assert_some!(&coins_list);
        let CoingeckoCoinsList { etag, .. } = coins_list.unwrap();
        let coins_list = client.get_all_metadata(etag.as_ref()).await?;
        assert_none!(coins_list);
        Ok(())
    }
}
