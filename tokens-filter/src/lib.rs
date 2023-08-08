pub mod coingecko;
pub mod coinmarketcap;
pub mod json;
pub mod jupiter;
pub mod solana;

#[async_trait::async_trait]
pub trait CheckToken {
    type Token;

    /// Check if the token is available to use in KeyApp
    /// algorithm: https://p2pvalidator.atlassian.net/wiki/spaces/Wallet/pages/2751168513/Scam+Token+Filtering+v2
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool>;
}
