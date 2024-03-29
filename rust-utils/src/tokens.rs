use anyhow::bail;
use borsh::BorshDeserialize;
use reqwest::StatusCode;
use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::Path,
    str::FromStr,
    sync::{Arc, RwLock, RwLockReadGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::{ParsePubkeyError, Pubkey};

use crate::error::{FeeTokenProviderError, UtilsError, UtilsResult};

static METADATA_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

fn serialize_pubkey<S>(input: &Pubkey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&input.to_string())
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    Pubkey::from_str(<String as Deserialize>::deserialize(deserializer)?.as_str()).map_err(|err| match err {
        ParsePubkeyError::WrongSize => serde::de::Error::custom("String is the wrong size"),
        ParsePubkeyError::Invalid => serde::de::Error::custom("Invalid Base58 string"),
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FeeToken {
    name: String,

    code: String,

    #[serde(serialize_with = "serialize_pubkey", deserialize_with = "deserialize_pubkey")]
    mint: Pubkey,

    #[serde(serialize_with = "serialize_pubkey", deserialize_with = "deserialize_pubkey")]
    account: Pubkey,

    exchange_rate: f64,

    is_update_failed: bool,
}

impl FeeToken {
    pub fn new(
        name: impl Into<String>,
        code: impl Into<String>,
        mint: Pubkey,
        account: Pubkey,
        exchange_rate: f64,
    ) -> Self {
        Self {
            name: name.into(),
            code: code.into(),
            mint,
            account,
            exchange_rate,
            is_update_failed: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn mint(&self) -> &Pubkey {
        &self.mint
    }

    pub fn account(&self) -> &Pubkey {
        &self.account
    }

    pub fn exchange_rate(&self) -> f64 {
        self.exchange_rate
    }

    pub fn is_update_failed(&self) -> bool {
        self.is_update_failed
    }
}

#[derive(Default, Clone)]
pub struct FeeTokenProvider(Arc<RwLock<HashMap<Pubkey, FeeToken>>>);

impl FeeTokenProvider {
    pub fn load(&self, config_path: impl AsRef<Path>) -> UtilsResult<()> {
        let file = File::open(config_path)?;
        let reader = BufReader::new(file);
        let tokens: Vec<FeeToken> = serde_json::from_reader(reader)?;

        let mut fee_tokens = HashMap::new();
        for token in tokens {
            if fee_tokens.contains_key(&token.mint) {
                return Err(UtilsError::FeeTokenProviderError(
                    FeeTokenProviderError::DuplicateTokenMint(token.mint.to_string()),
                ));
            }
            fee_tokens.insert(token.mint, token);
        }

        *(self.0.write().map_err(|_| poison_error())?) = fee_tokens;

        Ok(())
    }

    pub fn save(&self, config_path: impl AsRef<Path> + std::fmt::Display) -> UtilsResult<()> {
        let tmp_path_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| {
                FeeTokenProviderError::PoisonError(format!("Failed to create suffix for temporary file: {err}"))
            })?
            .as_millis();

        let temporary_file = format!("{config_path}_tmp_{tmp_path_suffix:?}");

        let contents = self
            .0
            .read()
            .map_err(|_| poison_error())?
            .iter()
            .map(|(_, fee_token)| fee_token)
            .cloned()
            .collect::<Vec<_>>();

        let contents = serde_json::to_string_pretty(&contents)?;

        std::fs::write(&temporary_file, contents)?;
        std::fs::rename(temporary_file, config_path)?;

        Ok(())
    }

    pub fn is_empty(&self) -> UtilsResult<bool> {
        Ok(self.0.read().map_err(|_| poison_error())?.is_empty())
    }

    pub fn get(&self, mint: &Pubkey) -> UtilsResult<Option<FeeToken>> {
        Ok(self.0.read().map_err(|_| poison_error())?.get(mint).cloned())
    }

    pub fn get_by_account(&self, account: &Pubkey) -> UtilsResult<Option<FeeToken>> {
        Ok(self
            .0
            .read()
            .map_err(|_| poison_error())?
            .values()
            .find(|token| token.account == *account)
            .cloned())
    }

    pub fn update_exchange_rates(&self, tokens_price: &HashMap<String, f64>) -> UtilsResult<()> {
        self.0
            .write()
            .map_err(|_| poison_error())?
            .iter_mut()
            .for_each(|(_, fee_token)| match tokens_price.get(fee_token.name()) {
                Some(new_exchange_rate) => {
                    fee_token.exchange_rate = *new_exchange_rate;
                    fee_token.is_update_failed = false;
                },
                None => {
                    log::error!(
                        "Unable to update exchange_rate for {}: token not found",
                        fee_token.name()
                    );
                    fee_token.is_update_failed = true
                },
            });

        Ok(())
    }

    pub fn read(&self) -> UtilsResult<RwLockReadGuard<HashMap<Pubkey, FeeToken>>> {
        Ok(self.0.read().map_err(|_| poison_error())?)
    }

    pub fn contains_token(&self, mint: &Pubkey) -> UtilsResult<bool> {
        Ok(self.0.read().map_err(|_| poison_error())?.contains_key(mint))
    }

    pub fn contains_active_token(&self, mint: &Pubkey) -> UtilsResult<bool> {
        Ok(self
            .0
            .read()
            .map_err(|_| poison_error())?
            .get(mint)
            .map(|token| !token.is_update_failed)
            .unwrap_or(false))
    }
}

fn poison_error() -> FeeTokenProviderError {
    FeeTokenProviderError::PoisonError("FeeTokenProvider".into())
}

/// Get token symbol by mint for token-list
/// Deprecated since 2022-06
pub async fn get_token_symbol_by_mint_from_json(mint: &str) -> anyhow::Result<String> {
    let chain_id = "101"; // MAIN NET
    let target = format!("https://cdn.jsdelivr.net/gh/CLBExchange/certified-token-list/{chain_id}/{mint}.json");

    #[derive(Deserialize)]
    struct Response {
        symbol: String,
    }

    let response = reqwest::get(target).await?;

    match response.status() {
        StatusCode::NOT_FOUND => bail!("token not found"),
        StatusCode::OK => Ok(response.json::<Response>().await.map(|x: Response| x.symbol)?),
        _ => bail!("Unable to get token symbol: {}", response.status()),
    }
}

/// Get token symbol from Metaplex Fungible Token Metadata
/// https://docs.metaplex.com/programs/token-metadata/accounts#metadata
/// Recommended method since 2022-06
pub async fn get_token_symbol_by_mint_from_metadata(client: &RpcClient, mint: &Pubkey) -> anyhow::Result<String> {
    let (metadata_address, _) = Pubkey::find_program_address(
        &[b"metadata", METADATA_PROGRAM_ID.as_ref(), mint.as_ref()],
        &METADATA_PROGRAM_ID,
    );
    let metadata = client.get_account_data(&metadata_address).await?;

    // The on-chain symbol of the token, limited to 10 bytes
    // Offset - 101, size 14
    let symbol = String::try_from_slice(&metadata[101..115])?;

    Ok(symbol.trim_end_matches('\0').to_owned())
}

pub async fn get_token_symbol_by_mint(client: &RpcClient, mint: &Pubkey) -> anyhow::Result<String> {
    match get_token_symbol_by_mint_from_metadata(client, mint).await {
        Ok(symbol) => Ok(symbol),
        Err(error) => {
            log::warn!(
                "unable to get token name for mint '{}' from on-chain metadata, fallback to token-list: {error}",
                mint
            );
            get_token_symbol_by_mint_from_json(&mint.to_string()).await
        },
    }
}

#[cfg(test)]
mod tests {
    use claim::{assert_err, assert_ok_eq};
    use solana_client::nonblocking::rpc_client::RpcClient;
    use std::{
        collections::HashMap,
        str::FromStr,
        sync::{Arc, RwLock},
    };

    use solana_sdk::pubkey::Pubkey;

    use crate::tokens::{
        get_token_symbol_by_mint, get_token_symbol_by_mint_from_json, get_token_symbol_by_mint_from_metadata,
    };

    use super::{FeeToken, FeeTokenProvider};

    fn init_fee_token_provider(is_update_failed: bool) -> FeeTokenProvider {
        let mut fee_tokens = HashMap::new();
        for i in 0..3 {
            let mint = Pubkey::new_unique();
            let fee_token = FeeToken {
                name: format!("token{i}"),
                code: format!("tkn{i}"),
                mint,
                account: Pubkey::new_unique(),
                exchange_rate: i as f64,
                is_update_failed,
            };
            fee_tokens.insert(mint, fee_token);
        }

        FeeTokenProvider(Arc::new(RwLock::new(fee_tokens)))
    }

    #[tokio::test]
    async fn get_tokens_symbol_by_mint_from_json() {
        assert_ok_eq!(
            get_token_symbol_by_mint_from_json("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs").await,
            "ETH".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint_from_json("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").await,
            "USDC".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint_from_json("9n4nbM75f5Ui33ZbPYXn59EwSgE8CGsHtAeTH5YFeJ9E").await,
            "BTC".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint_from_json("EzfnjRUKtc5vweE1GCLdHV4MkDQ3ebSpQXLobSKgQ9RB").await,
            "CSM".to_string()
        );
    }

    #[tokio::test]
    async fn get_tokens_symbol_by_mint_from_metadata() {
        let solana_client = RpcClient::new("https://api.mainnet-beta.solana.com".into());

        // token-list has ETH where as metadata hs WETH
        assert_ok_eq!(
            get_token_symbol_by_mint_from_metadata(
                &solana_client,
                &Pubkey::from_str("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs").unwrap()
            )
            .await,
            "WETH".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint_from_metadata(
                &solana_client,
                &Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap()
            )
            .await,
            "USDC".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint_from_metadata(
                &solana_client,
                &Pubkey::from_str("9n4nbM75f5Ui33ZbPYXn59EwSgE8CGsHtAeTH5YFeJ9E").unwrap()
            )
            .await,
            "BTC".to_string()
        );

        // This mint doesn't have metadata
        assert_err!(
            get_token_symbol_by_mint_from_metadata(
                &solana_client,
                &Pubkey::from_str("EzfnjRUKtc5vweE1GCLdHV4MkDQ3ebSpQXLobSKgQ9RB").unwrap()
            )
            .await
        );
    }

    #[tokio::test]
    async fn get_tokens_symbol_by_mint_with_fallback() {
        let solana_client = RpcClient::new("https://api.mainnet-beta.solana.com".into());

        // token-list has ETH where as metadata hs WETH
        assert_ok_eq!(
            get_token_symbol_by_mint(
                &solana_client,
                &Pubkey::from_str("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs").unwrap()
            )
            .await,
            "WETH".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint(
                &solana_client,
                &Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap()
            )
            .await,
            "USDC".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint(
                &solana_client,
                &Pubkey::from_str("9n4nbM75f5Ui33ZbPYXn59EwSgE8CGsHtAeTH5YFeJ9E").unwrap()
            )
            .await,
            "BTC".to_string()
        );

        assert_ok_eq!(
            get_token_symbol_by_mint(
                &solana_client,
                &Pubkey::from_str("EzfnjRUKtc5vweE1GCLdHV4MkDQ3ebSpQXLobSKgQ9RB").unwrap()
            )
            .await,
            "CSM".to_string()
        );
    }

    #[test]
    fn update_exchange_rates_successfully_3_out_of_3() {
        let fee_token_provider = init_fee_token_provider(false);

        let new_prices = HashMap::from([
            ("token0".to_string(), 1f64),
            ("token1".to_string(), 2f64),
            ("token2".to_string(), 3f64),
        ]);

        fee_token_provider
            .update_exchange_rates(&new_prices)
            .expect("shouldn't fail");

        fee_token_provider
            .0
            .read()
            .expect("Shouldn't be blocked in test")
            .iter()
            .for_each(|(_, fee_token)| {
                match fee_token.name() {
                    "token0" => {
                        assert_eq!("tkn0", fee_token.code());
                        assert_eq!(1f64, fee_token.exchange_rate());
                    },
                    "token1" => {
                        assert_eq!("tkn1", fee_token.code());
                        assert_eq!(2f64, fee_token.exchange_rate());
                    },
                    "token2" => {
                        assert_eq!("tkn2", fee_token.code());
                        assert_eq!(3f64, fee_token.exchange_rate());
                    },
                    _ => panic!("Fee token with name '{}' not found", fee_token.name()),
                }
                assert!(!fee_token.is_update_failed());
            });
    }

    #[test]
    fn update_exchange_rates_successfully_2_out_of_3() {
        let fee_token_provider = init_fee_token_provider(true);

        let new_prices = HashMap::from([("token0".to_string(), 1f64), ("token2".to_string(), 3f64)]);

        fee_token_provider
            .update_exchange_rates(&new_prices)
            .expect("shouldn't fail");

        fee_token_provider
            .0
            .read()
            .expect("Shouldn't be blocked in test")
            .iter()
            .for_each(|(_, fee_token)| match fee_token.name() {
                "token0" => {
                    assert_eq!("tkn0", fee_token.code());
                    assert_eq!(1f64, fee_token.exchange_rate());
                    assert!(!fee_token.is_update_failed());
                },
                "token1" => {
                    assert_eq!("tkn1", fee_token.code());
                    assert_eq!(1f64, fee_token.exchange_rate());
                    assert!(fee_token.is_update_failed());
                },
                "token2" => {
                    assert_eq!("tkn2", fee_token.code());
                    assert_eq!(3f64, fee_token.exchange_rate());
                    assert!(!fee_token.is_update_failed());
                },
                _ => panic!("Fee token with name '{}' not found", fee_token.name()),
            });
    }
}
