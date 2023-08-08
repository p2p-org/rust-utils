use std::{cell::OnceCell, collections::HashMap, sync::Arc};

use async_trait::async_trait;
use cached::{Cached, TimedCache};
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::Mutex;

use crate::CheckToken;

pub static DEFAULT_URL: &str = "https://cache.jup.ag/indexed-route-maps-v3";
pub const SOL: OnceCell<String> = OnceCell::new();

pub struct RoutesCache(TimedCache<String, usize>);

impl RoutesCache {
    fn update_from_json(&mut self, input: RawResponse) {
        input
            .mint_keys
            .into_iter()
            .zip(input.indexed_route_map.into_iter())
            .for_each(|(mint_key, (_, routes))| {
                let routes_count = routes.len();
                self.0.cache_set(mint_key, routes_count);
            });
    }

    /// Check if json cache is updated via SOL routes. Because SOL token always should be in json.
    fn is_updated(&self) -> bool {
        let lifespan = self.0.cache_lifespan().unwrap_or_default();

        self.0
            .get_store()
            .get(SOL.get_or_init(|| spl_token::native_mint::id().to_string()))
            .map(|(instant, _)| instant.elapsed().as_secs() < lifespan)
            .unwrap_or_default()
    }
}

impl From<(RawResponse, u64)> for RoutesCache {
    fn from(input: (RawResponse, u64)) -> Self {
        let value = input.0;
        let ttl = input.1;

        let mut cache = Self(TimedCache::with_lifespan_and_capacity(ttl, value.mint_keys.len()));
        cache.update_from_json(value);
        cache
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawResponse {
    mint_keys: Vec<String>,
    indexed_route_map: HashMap<i32, Vec<i32>>,
}

pub struct JupiterChecker {
    url: String,
    cache: Mutex<RoutesCache>,
}

impl JupiterChecker {
    async fn get_from_cache_or_update(&self, key: String) -> anyhow::Result<usize> {
        let mut guard = self.cache.lock().await;

        if let Some(routes_count) = guard.0.cache_get(&key) {
            return Ok(*routes_count);
        }

        if !guard.is_updated() {
            tracing::debug!(key, "cache expired");
            let new_json = Self::get_json(&self.url).await?;
            guard.update_from_json(new_json);
        }

        Ok(*guard.0.cache_get_or_set_with(key, || 0))
    }

    async fn get_json(url: &str) -> anyhow::Result<RawResponse> {
        Ok(reqwest::get(url).await?.json().await?)
    }

    pub async fn new(url: String, ttl: u64) -> anyhow::Result<Self> {
        Ok(Self {
            cache: Mutex::new((Self::get_json(&url).await?, ttl).into()),
            url,
        })
    }
}

#[async_trait]
impl CheckToken for JupiterChecker {
    type Token = Pubkey;

    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        Ok(self.get_from_cache_or_update(token.to_string()).await? > 0)
    }
}

#[async_trait]
impl CheckToken for Arc<JupiterChecker> {
    type Token = Pubkey;

    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        Ok(self.as_ref().get_from_cache_or_update(token.to_string()).await? > 0)
    }
}

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey;

    use super::*;

    #[tokio::test]
    #[ignore = "needs to mock jupiter json"]
    async fn check() {
        let client = Arc::new(JupiterChecker::new(DEFAULT_URL.to_owned(), 2).await.unwrap());

        let good = client
            .check_token(&pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")) // USDC
            .await
            .unwrap();
        assert!(good);
        let bad = client.check_token(&Pubkey::new_unique()).await.unwrap();
        assert!(!bad);
    }
}
