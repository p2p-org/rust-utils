use std::sync::Arc;

use anyhow::Ok;
use async_trait::async_trait;
use coingecko_client::CoingeckoClient;
use coinmarketcap_client::CoinmarketcapClient;
use derive_more::From;
use http_client::settings::HttpClientSettings;
use permissions_list::PermissionsList;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use crate::{json::JsonChecker, jupiter::JupiterChecker};

pub mod coingecko;
pub mod coinmarketcap;
pub mod json;
pub mod jupiter;
pub mod permissions_list;
pub mod solana;

#[derive(From)]
pub enum Checker {
    #[from]
    Json(JsonChecker),
    #[from]
    Coinmarketcap(CoinmarketcapClient),
    #[from]
    Coingecko(CoingeckoClient),
    #[from]
    Jupiter(JupiterChecker),
    #[from]
    Solana(Arc<RpcClient>),
}

impl std::fmt::Display for Checker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Checker::Json(_) => "Json",
            Checker::Coinmarketcap(_) => "Coinmarketcap",
            Checker::Coingecko(_) => "Coingecko",
            Checker::Jupiter(_) => "Jupiter",
            Checker::Solana(_) => "Solana",
        };

        f.write_str(msg)
    }
}

#[async_trait]
impl CheckToken for Checker {
    type Token = Pubkey;

    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        match self {
            Checker::Json(x) => x.check_token(token),
            Checker::Coinmarketcap(x) => x.check_token(token),
            Checker::Coingecko(x) => x.check_token(token),
            Checker::Jupiter(x) => x.check_token(token),
            Checker::Solana(x) => x.check_token(token),
        }
        .await
    }
}

#[async_trait]
pub trait CheckToken {
    type Token;

    /// Check if the token is available to use in KeyApp
    /// algorithm: https://p2pvalidator.atlassian.net/wiki/spaces/Wallet/pages/2751168513/Scam+Token+Filtering+v2
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool>;
}

#[derive(Default)]
pub struct TokensFilter {
    permissions_list: PermissionsList,
    checkers: Vec<Checker>,
}

impl TokensFilter {
    /// According this scheme https://drive.google.com/file/d/1IZl5Y4iREI666ffcRoBaU8310z3XnECh/view the order of checkers will be:
    /// coingecko - github_json - coinmarketcap - jupiter (solana_rpc not required now)
    pub async fn new_all(
        coingecko_settings: HttpClientSettings,
        coinmarketcap_settings: HttpClientSettings,
        jupiter_url: String,
        ttl: u64,
    ) -> anyhow::Result<Self> {
        Self::default()
            .with_coingecko(coingecko_settings)?
            .with_json()
            .with_coinmarketcap(coinmarketcap_settings)
            .with_jupiter(jupiter_url, ttl)
            .await
    }

    pub fn with_json(mut self) -> Self {
        let checker = JsonChecker;
        self.checkers.push(checker.into());
        self
    }

    pub fn with_coinmarketcap(mut self, coinmarketcap_settings: HttpClientSettings) -> Self {
        let checker = CoinmarketcapClient::new(coinmarketcap_settings);
        self.checkers.push(checker.into());
        self
    }

    pub async fn with_jupiter(mut self, jupiter_url: String, ttl: u64) -> anyhow::Result<Self> {
        let checker = JupiterChecker::new(jupiter_url, ttl).await?;
        self.checkers.push(checker.into());
        Ok(self)
    }

    pub fn with_coingecko(mut self, coingecko_settings: HttpClientSettings) -> anyhow::Result<Self> {
        let checker = CoingeckoClient::new(coingecko_settings)?;
        self.checkers.push(checker.into());
        Ok(self)
    }

    pub fn with_solana_rpc(mut self, client: Arc<RpcClient>) -> Self {
        self.checkers.push(client.into());
        self
    }

    pub fn with_solana(mut self, url: String) -> Self {
        let client = Arc::new(RpcClient::new(url));
        self.checkers.push(client.into());
        self
    }

    pub fn with_permissions_list(mut self, permissions_list: PermissionsList) -> Self {
        self.permissions_list = permissions_list;
        self
    }
}

#[async_trait]
impl CheckToken for TokensFilter {
    type Token = Pubkey;

    #[tracing::instrument(skip(self))]
    async fn check_token(&self, token: &Self::Token) -> anyhow::Result<bool> {
        if self.permissions_list.is_blacklisted(token) {
            tracing::debug!(?token, "token is blacklisted");
            return Ok(false);
        }

        if self.permissions_list.is_whitelisted(token) {
            tracing::debug!(?token, "token is whitelisted");
            return Ok(true);
        }

        for checker in &self.checkers {
            if checker.check_token(token).await? {
                tracing::debug!(?token, %checker, "token is checked");
                return Ok(true);
            }
        }

        tracing::debug!(?token, "token is not checked");
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use rust_utils::telemetry::{make_resource, Telemetry, TracingSettings};
    use solana_sdk::pubkey;

    use super::*;

    pub const WHITELISTED_TOKEN: Pubkey = pubkey!("F4SjgUSDx2XqkiNzvX74ANRTTGCWGv5qDcShF8HtMMqd");
    pub const BLACKLISTED_TOKEN: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"); // USDC

    // Source: https://docs.google.com/spreadsheets/d/1yO6CwMhXpPApkwHXc04_38UdxXv7-2T-Kl0IefB6Vo0/edit#gid=77498325
    pub const NOT_SCAM: [Pubkey; 20] = [
        pubkey!("5MAYDfq5yxtudAhtfyuMBuHZjgAbaS9tbEyEQYAhDS5y"),
        pubkey!("AUrMpCDYYcPuHhyNX8gEEqbmDPFUpBpHrNW3vPeCFn5Z"),
        pubkey!("7A4DPNz5rUZhHrAeRQ9C5yPivmVVnedJfTayYwrxCi7i"),
        pubkey!("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"),
        pubkey!("CooKrc8eaU4E6AxBtNqA14V5HWCtxqeY7tZ7k2rS6QGP"),
        pubkey!("o1Mw5Y3n68o8TakZFuGKLZMGjm72qv4JeoZvGiCLEvK"),
        pubkey!("5yxNbU8DgYJZNi3mPD9rs4XLh9ckXrhPjJ5VCujUWg5H"),
        pubkey!("fYHm99irG7HdL47gdfeb4WJUpfYDoQsmnYaoBa9QpsC"),
        pubkey!("hntyVP6YFm1Hg25TN9WGLqM12b8TQmcknKrdu1oxWux"),
        pubkey!("PhiLR4JDZB9z92rYT5xBXKCxmq4pGB1LYjtybii7aiS"),
        pubkey!("H1G6sZ1WDoMmMCFqBKAbg9gkQPCo1sKQtaJWz9dHmqZr"),
        pubkey!("3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh"),
        pubkey!("yomFPUqz1wJwYSfD5tZJUtS3bNb8xs8mx9XzBv8RL39"),
        pubkey!("CDJWUqTcYTVAKXAVXoQZFes5JUFc7owSeq7eMQcDSbo5"),
        pubkey!("ZScHuTtqZukUrtZS43teTKGs2VqkKL8k4QCouR2n6Uo"),
        pubkey!("ax7EjwgRaerUacfCAptcMnkckmf8Wiee5T9KLqSzsF6"),
        pubkey!("HHjoYwUp5aU6pnrvN4s2pwEErwXNZKhxKGYjRJMoBjLw"),
        pubkey!("GKNr1Gwf7AMvEMEyMzBoEALVBvCpKJue9Lzn9HfrYYhg"),
        pubkey!("5tB5D6DGJMxxHYmNkfJNG237x6pZGEwTzGpUUh62yQJ7"),
        pubkey!("76ijxiMkj4DX8q9QMtqpzTxFnT4KPmWv47sZf2kKoVwk"),
    ];

    pub const SCAM: [Pubkey; 3] = [
        pubkey!("5iqpxTw7e9CXtopKmh7cp4n6p58gbGGg8YcX8LKAEWSi"),
        pubkey!("J16p4izHspZPne3zcGAgP4c23tkFuVd67MoNfcAMXNwu"),
        pubkey!("Bsw98fp1Ef2E9PUNgd423YmFEQx6yE2ht7rWKdxA9VVW"),
    ];

    #[tokio::test]
    #[ignore = "integration test"]
    async fn check_filter() {
        let tracing = TracingSettings {
            spec: "debug".into(),
            ..Default::default()
        };

        let (_, subscriber) = Telemetry::init(make_resource("TEST", env!("CARGO_PKG_VERSION")), tracing).unwrap();
        Telemetry::init_subscriber(subscriber).unwrap();

        let filter = TokensFilter::new_all(
            HttpClientSettings::default(),
            HttpClientSettings {
                api_key: Some("...".into()),
                ..Default::default()
            },
            jupiter::DEFAULT_URL.to_owned(),
            120,
        )
        .await
        .unwrap();

        for token in NOT_SCAM.iter() {
            let r = filter.check_token(token).await.unwrap();
            assert!(r, "token: {}", token);
            tokio::time::sleep(std::time::Duration::from_secs(10)).await; // Coingecko API limit
        }

        for token in SCAM.iter() {
            let r = filter.check_token(token).await.unwrap();
            assert!(!r, "token: {}", token);
            tokio::time::sleep(std::time::Duration::from_secs(10)).await; // Coingecko API limit
        }

        // before added permission list
        assert!(!filter.check_token(&WHITELISTED_TOKEN).await.unwrap());
        assert!(filter.check_token(&BLACKLISTED_TOKEN).await.unwrap());

        let pl = PermissionsList::new(
            [(WHITELISTED_TOKEN, true), (BLACKLISTED_TOKEN, false)]
                .into_iter()
                .collect(),
        );

        let filter = filter.with_permissions_list(pl);

        assert!(filter.check_token(&WHITELISTED_TOKEN).await.unwrap());
        assert!(!filter.check_token(&BLACKLISTED_TOKEN).await.unwrap());
    }
}
