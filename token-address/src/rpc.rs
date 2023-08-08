use crate::{ChainId, StoredTokenAddress};
use hex_literal::hex;
use primitive_types::H160;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use solana_sdk::{pubkey, pubkey::Pubkey};
use std::{fmt, fmt::Formatter};

const WRAPPED_SOL_STR: &str = "So11111111111111111111111111111111111111112";
const WRAPPED_SOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const WRAPPED_ETH_STR: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const WRAPPED_ETH_ADDRESS: [u8; 20] = hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

// https://github.com/serde-rs/serde/issues/1560#issuecomment-506915291
macro_rules! named_unit_variant {
    ($variant:ident) => {
        pub mod $variant {
            pub fn serialize<S>(serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(stringify!($variant))
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = ();
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(concat!("\"", stringify!($variant), "\""))
                    }
                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        if value == stringify!($variant) {
                            Ok(())
                        } else {
                            Err(E::invalid_value(serde::de::Unexpected::Str(value), &self))
                        }
                    }
                }
                deserializer.deserialize_str(V)
            }
        }
    };
}

mod strings {
    named_unit_variant!(native);
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum RawTokenAddress {
    Array([u8; 32]),
    Spl(#[serde_as(as = "DisplayFromStr")] Pubkey),
    Erc20(H160),
    #[serde(with = "strings::native")]
    Native,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(from = "RawTokenAddress", into = "RawTokenAddress")]
pub enum TokenAddress {
    Spl(Pubkey),
    Erc20(H160),
    Native(ChainId),
}

impl From<Pubkey> for TokenAddress {
    fn from(value: Pubkey) -> Self {
        TokenAddress::Spl(value)
    }
}

impl From<H160> for TokenAddress {
    fn from(value: H160) -> Self {
        TokenAddress::Erc20(value)
    }
}

impl From<RawTokenAddress> for TokenAddress {
    fn from(value: RawTokenAddress) -> Self {
        match value {
            RawTokenAddress::Array(pubkey) => TokenAddress::Spl(pubkey.into()),
            RawTokenAddress::Spl(pubkey) => TokenAddress::Spl(pubkey),
            RawTokenAddress::Erc20(address) => TokenAddress::Erc20(address),
            RawTokenAddress::Native => TokenAddress::Native(ChainId::Solana),
        }
    }
}

impl From<TokenAddress> for RawTokenAddress {
    fn from(value: TokenAddress) -> Self {
        match value {
            TokenAddress::Spl(pubkey) => RawTokenAddress::Spl(pubkey),
            TokenAddress::Erc20(address) => RawTokenAddress::Erc20(address),
            TokenAddress::Native(_) => RawTokenAddress::Native,
        }
    }
}

impl fmt::Display for TokenAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TokenAddress::Spl(pubkey) => write!(f, "{pubkey}"),
            TokenAddress::Erc20(address) => write!(f, "0x{address:x}"),
            TokenAddress::Native(ChainId::Solana) => f.write_str(WRAPPED_SOL_STR),
            TokenAddress::Native(ChainId::Ethereum) => f.write_str(WRAPPED_ETH_STR),
        }
    }
}

pub enum StoredTokenAddressExtra {
    StoredTokenAddress(StoredTokenAddress),
    Native(ChainId),
}

impl TokenAddress {
    pub fn spl(&self) -> Option<Pubkey> {
        match self {
            TokenAddress::Spl(pubkey) => Some(*pubkey),
            _ => None,
        }
    }

    pub fn erc20(&self) -> Option<H160> {
        match self {
            TokenAddress::Erc20(address) => Some(*address),
            _ => None,
        }
    }

    pub fn as_solana_address(&self) -> Option<SolanaAddress> {
        match self {
            TokenAddress::Spl(address) => Some(SolanaAddress::Spl(*address)),
            TokenAddress::Native(ChainId::Solana) => Some(SolanaAddress::Native),
            _ => None,
        }
    }

    pub fn as_ethereum_address(&self) -> Option<EthereumAddress> {
        match self {
            TokenAddress::Erc20(address) => Some(EthereumAddress::Erc20(*address)),
            TokenAddress::Native(ChainId::Ethereum) => Some(EthereumAddress::Native),
            _ => None,
        }
    }

    pub fn as_stored_token_address(&self) -> Option<StoredTokenAddress> {
        match self {
            TokenAddress::Spl(pubkey) => Some(StoredTokenAddress::Solana(*pubkey)),
            TokenAddress::Erc20(address) => Some(StoredTokenAddress::Ethereum(*address)),
            TokenAddress::Native(_) => None,
        }
    }

    pub fn platform(&self) -> ChainId {
        match self {
            TokenAddress::Spl(_) => ChainId::Solana,
            TokenAddress::Erc20(_) => ChainId::Ethereum,
            TokenAddress::Native(chain_id) => *chain_id,
        }
    }
}

impl From<&TokenAddress> for StoredTokenAddressExtra {
    fn from(value: &TokenAddress) -> Self {
        match value {
            TokenAddress::Spl(address) => {
                StoredTokenAddressExtra::StoredTokenAddress(StoredTokenAddress::Solana(*address))
            },
            TokenAddress::Erc20(address) => {
                StoredTokenAddressExtra::StoredTokenAddress(StoredTokenAddress::Ethereum(*address))
            },
            TokenAddress::Native(chain_id) => StoredTokenAddressExtra::Native(*chain_id),
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum RawSolanaAddress {
    Array([u8; 32]),
    Spl(#[serde_as(as = "DisplayFromStr")] Pubkey),
    #[serde(with = "strings::native")]
    Native,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(from = "RawSolanaAddress", into = "RawSolanaAddress")]
pub enum SolanaAddress {
    Spl(Pubkey),
    Native,
}

impl fmt::Display for SolanaAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SolanaAddress::Spl(pubkey) => pubkey.fmt(f),
            SolanaAddress::Native => f.write_str(WRAPPED_SOL_STR),
        }
    }
}

impl From<RawSolanaAddress> for SolanaAddress {
    fn from(value: RawSolanaAddress) -> Self {
        match value {
            RawSolanaAddress::Array(mint) => SolanaAddress::Spl(mint.into()),
            RawSolanaAddress::Spl(mint) => SolanaAddress::Spl(mint),
            RawSolanaAddress::Native => SolanaAddress::Native,
        }
    }
}

impl From<SolanaAddress> for RawSolanaAddress {
    fn from(value: SolanaAddress) -> Self {
        match value {
            SolanaAddress::Spl(mint) => RawSolanaAddress::Spl(mint),
            SolanaAddress::Native => RawSolanaAddress::Native,
        }
    }
}

impl From<Pubkey> for SolanaAddress {
    fn from(value: Pubkey) -> Self {
        SolanaAddress::Spl(value)
    }
}

impl From<SolanaAddress> for TokenAddress {
    fn from(value: SolanaAddress) -> Self {
        match value {
            SolanaAddress::Spl(mint) => TokenAddress::Spl(mint),
            SolanaAddress::Native => TokenAddress::Native(ChainId::Solana),
        }
    }
}

impl SolanaAddress {
    pub fn pubkey(&self) -> Pubkey {
        match self {
            SolanaAddress::Spl(key) => *key,
            SolanaAddress::Native => WRAPPED_SOL,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(untagged)]
pub enum EthereumAddress {
    Erc20(H160),
    #[serde(with = "strings::native")]
    Native,
}

impl From<H160> for EthereumAddress {
    fn from(value: H160) -> Self {
        EthereumAddress::Erc20(value)
    }
}

impl From<EthereumAddress> for TokenAddress {
    fn from(value: EthereumAddress) -> Self {
        match value {
            EthereumAddress::Erc20(address) => TokenAddress::Erc20(address),
            EthereumAddress::Native => TokenAddress::Native(ChainId::Ethereum),
        }
    }
}

impl fmt::Display for EthereumAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EthereumAddress::Erc20(address) => write!(f, "0x{address:x}"),
            EthereumAddress::Native => f.write_str(WRAPPED_ETH_STR),
        }
    }
}

impl EthereumAddress {
    pub fn address(&self) -> H160 {
        match self {
            EthereumAddress::Erc20(address) => *address,
            EthereumAddress::Native => WRAPPED_ETH_ADDRESS.into(),
        }
    }

    pub fn wrapped_eth() -> Self {
        EthereumAddress::Erc20(WRAPPED_ETH_ADDRESS.into())
    }
}

#[cfg(test)]
mod test {
    use crate::{rpc::TokenAddress, ChainId};
    use primitive_types::H160;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn should_serde_solana_pubkey() {
        let input = [0u8; 32];
        let serialized = serde_json::to_string(&input).unwrap();
        assert_eq!(
            serialized,
            "[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]"
        );
        let deserialized: TokenAddress = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TokenAddress::Spl(input.into()));

        let input = Pubkey::new_unique();
        let serialized = format!("\"{pubkey}\"", pubkey = input.to_string());
        let deserialized: TokenAddress = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TokenAddress::Spl(input));
    }

    #[test]
    fn should_serde_ethereum_address() {
        let address = H160::random();
        let serialized = serde_json::to_string(&address).unwrap();
        let deserialized: TokenAddress = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TokenAddress::Erc20(address));
    }

    #[test]
    fn should_serde_native() {
        let serialized = serde_json::to_string(&TokenAddress::Native(ChainId::Solana)).unwrap();
        let deserialized: TokenAddress = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TokenAddress::Native(ChainId::Solana));
    }
}
