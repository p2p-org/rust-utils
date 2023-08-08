use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use normdecimal::NormDecimal;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use crate::CheckToken;

pub const NFT_AMOUNT: NormDecimal = NormDecimal::ONE;
pub const NFT_DECIMALS: u8 = 0;

#[async_trait]
impl CheckToken for RpcClient {
    type Token = Pubkey;

    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        self.get_token_supply(token).await.map(|supply| {
            let amount = supply.ui_amount_string.parse::<NormDecimal>().with_context(|| {
                format!(
                    "Unable to parse ui_amount_string({}) to Decimal",
                    supply.ui_amount_string
                )
            })?;

            Ok::<_, anyhow::Error>(amount == NFT_AMOUNT && supply.decimals == NFT_DECIMALS || amount > NFT_AMOUNT)
        })?
    }
}

#[async_trait]
impl CheckToken for Arc<RpcClient> {
    type Token = Pubkey;

    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        self.as_ref().check_token(token).await
    }
}

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey;

    use super::*;

    #[tokio::test]
    #[ignore = "needs to mock RpcClient"]
    async fn check() {
        let solana_client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".into()));

        let good = solana_client
            .check_token(&pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")) // USDC
            .await
            .unwrap();
        assert!(good);

        let nft = solana_client
            .check_token(&pubkey!("J7W8hKLg9a8KUsZu5wmMo6pufocW9qhRUR7Fx7isRoYU"))
            .await
            .unwrap();
        assert!(nft);

        let error = solana_client.check_token(&Pubkey::new_unique()).await.is_err();
        assert!(error);
    }
}
