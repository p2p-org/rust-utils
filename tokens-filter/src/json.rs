use std::sync::Arc;

use async_trait::async_trait;
use rust_utils::tokens::get_token_symbol_by_mint_from_json;
use solana_sdk::pubkey::Pubkey;

use crate::CheckToken;

pub struct JsonChecker;

#[async_trait]
impl CheckToken for JsonChecker {
    type Token = Pubkey;

    #[tracing::instrument(skip(self), err)]
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        match get_token_symbol_by_mint_from_json(&token.to_string()).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("unable to parse response from token-list") {
                    Ok(false)
                } else {
                    Err(e)
                }
            },
        }
    }
}

#[async_trait]
impl CheckToken for Arc<JsonChecker> {
    type Token = Pubkey;

    #[tracing::instrument(skip(self), err)]
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        self.as_ref().check_token(token).await
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use solana_sdk::pubkey;

    use super::*;

    #[tokio::test]
    #[ignore = "integration test"]
    async fn check() {
        let client = Arc::new(JsonChecker);

        let good = client
            .check_token(&pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")) // USDC
            .await
            .unwrap();
        assert!(good);
        let not_found = client.check_token(&Pubkey::new_unique()).await.err().unwrap();
        assert_eq!(not_found.to_string(), anyhow!("token not found").to_string());
    }
}
