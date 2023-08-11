use async_trait::async_trait;
use coingecko_client::CoingeckoClient;
use solana_sdk::pubkey::Pubkey;
use token_address::StoredTokenAddress;

use crate::CheckToken;

#[async_trait]
impl CheckToken for CoingeckoClient {
    type Token = Pubkey;

    #[tracing::instrument(skip(self), err)]
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        Ok(self
            .get_metadata_by_address(&StoredTokenAddress::Solana(*token))
            .await?
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use http_client::settings::HttpClientSettings;
    use solana_sdk::pubkey;

    use super::*;

    #[tokio::test]
    #[ignore = "integration test"]
    async fn check() {
        let client = CoingeckoClient::new(HttpClientSettings::default()).unwrap();

        let good = client
            .check_token(&pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")) // USDC
            .await
            .unwrap();

        assert!(good);

        let bad = client.check_token(&Pubkey::new_unique()).await.unwrap();

        assert!(!bad);
    }
}
