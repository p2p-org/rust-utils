use serde::{Deserialize, Serialize};

pub mod db;
pub mod rpc;

pub use db::StoredTokenAddress;
pub use rpc::{EthereumAddress, SolanaAddress, TokenAddress};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Hash, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ChainId {
    Solana,
    Ethereum,
}
