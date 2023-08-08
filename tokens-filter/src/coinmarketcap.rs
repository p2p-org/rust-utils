use async_trait::async_trait;
use coinmarketcap_client::CoinmarketcapClient;
use solana_sdk::pubkey::Pubkey;

use crate::CheckToken;

#[async_trait]
impl CheckToken for CoinmarketcapClient {
    type Token = Pubkey;

    #[tracing::instrument(skip(self), err)]
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        let response = self.cryptocurrency_info(token.to_string()).await?;

        let Some(data) = response.get("data").and_then(|x| x.as_object()) else {
            tracing::debug!("No data in response");
            return Ok(false);
        };

        let Some((_, meta)) = data.iter().next() else {
            tracing::debug!("No metadata in response");
            return Ok(false);
        };

        let Some(symbol) = meta.get("symbol") else {
            tracing::debug!("No symbol in response");
            return Ok(false);
        };

        tracing::debug!(?symbol, "successful check");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use coinmarketcap_client::settings::HttpClientSettings;
    use solana_sdk::pubkey;

    use super::*;

    #[tokio::test]
    #[ignore = "setup api key"]
    async fn check() {
        let client = CoinmarketcapClient::new(&HttpClientSettings {
            api_key: Some("...".into()),
            ..Default::default()
        });

        let good = client
            .check_token(&pubkey!("7gjNiPun3AzEazTZoFEjZgcBMeuaXdpjHq2raZTmTrfs")) // CRV DAO
            .await
            .unwrap();

        assert!(good);

        let bad = client.check_token(&Pubkey::new_unique()).await.unwrap();

        assert!(!bad);
    }
}
