use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::Path,
    str::FromStr,
    sync::{Arc, RwLock, RwLockReadGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use solana_sdk::pubkey::{ParsePubkeyError, Pubkey};

use crate::error::{FeeTokenProviderError, UtilsError, UtilsResult};

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
    Pubkey::from_str(String::deserialize(deserializer)?.as_str()).map_err(|err| match err {
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
    pub fn new(name: impl Into<String>, code: impl Into<String>, mint: Pubkey, account: Pubkey, exchange_rate: f64) -> Self {
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
                FeeTokenProviderError::PoisonError(format!("Failed to create suffix for temporary file: {err}",))
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
            .collect_vec();

        let contents = serde_json::to_string_pretty(&contents)?;

        std::fs::write(&temporary_file, contents)?;
        std::fs::rename(temporary_file, config_path)?;

        Ok(())
    }

    pub fn is_empty(&self) -> UtilsResult<bool> {
        Ok(self.0.read().map_err(|_| poison_error())?.is_empty())
    }

    pub fn get(&self, key: &Pubkey) -> UtilsResult<Option<FeeToken>> {
        Ok(self.0.read().map_err(|_| poison_error())?.get(key).cloned())
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
        Ok(self
            .0
            .read()
            .map_err(|_| poison_error())?)
    }

    pub fn contains_key(&self, key: &Pubkey) -> UtilsResult<bool> {
        Ok(self.0.read().map_err(|_| poison_error())?.contains_key(key))
    }

    pub fn contains_active_token(&self, key: &Pubkey) -> UtilsResult<bool> {
        Ok(self.0.read().map_err(|_| poison_error())?.get(key).map(|token| !token.is_update_failed).unwrap_or(false))
    }
}

fn poison_error() -> FeeTokenProviderError {
    FeeTokenProviderError::PoisonError("FeeTokenProvider".into())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    };

    use solana_sdk::pubkey::Pubkey;

    use super::{FeeToken, FeeTokenProvider};

    fn init_fee_token_provider(is_update_failed: bool) -> FeeTokenProvider {
        let mut fee_tokens = HashMap::new();
        for i in 0..3 {
            let mint = Pubkey::new_unique();
            let fee_token = FeeToken {
                name: format!("token{i}"),
                code: format!("tkn{i}"),
                mint: mint.clone(),
                account: Pubkey::new_unique(),
                exchange_rate: i as f64,
                is_update_failed,
            };
            fee_tokens.insert(mint, fee_token);
        }

        FeeTokenProvider(Arc::new(RwLock::new(fee_tokens)))
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
                assert_eq!(false, fee_token.is_update_failed());
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
                    assert_eq!(false, fee_token.is_update_failed());
                },
                "token1" => {
                    assert_eq!("tkn1", fee_token.code());
                    assert_eq!(1f64, fee_token.exchange_rate());
                    assert_eq!(true, fee_token.is_update_failed());
                },
                "token2" => {
                    assert_eq!("tkn2", fee_token.code());
                    assert_eq!(3f64, fee_token.exchange_rate());
                    assert_eq!(false, fee_token.is_update_failed());
                },
                _ => panic!("Fee token with name '{}' not found", fee_token.name()),
            });
    }
}
