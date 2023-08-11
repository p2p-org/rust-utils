use crate::{
    rpc::{EthereumAddress, SolanaAddress, TokenAddress},
    ChainId,
};
use primitive_types::H160;
use solana_sdk::pubkey::Pubkey;
use sqlx::{
    database::{HasArguments, HasValueRef},
    encode::IsNull,
    error::BoxDynError,
    postgres::PgRow,
    Database, Decode, Encode, Error, FromRow, Row, Type,
};
use std::{fmt, fmt::Formatter, str::FromStr};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum StoredTokenAddress {
    Solana(Pubkey),
    Ethereum(H160),
}

impl From<StoredTokenAddress> for TokenAddress {
    fn from(value: StoredTokenAddress) -> Self {
        match value {
            StoredTokenAddress::Solana(address) => address.into(),
            StoredTokenAddress::Ethereum(address) => address.into(),
        }
    }
}

impl StoredTokenAddress {
    pub fn platform(&self) -> ChainId {
        match self {
            Self::Solana(_) => ChainId::Solana,
            Self::Ethereum(_) => ChainId::Ethereum,
        }
    }
}

impl fmt::Display for StoredTokenAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solana(pubkey) => write!(f, "{pubkey}",),
            Self::Ethereum(ethereum) => write!(f, "0x{ethereum:x}",),
        }
    }
}

impl From<EthereumAddress> for StoredTokenAddress {
    fn from(value: EthereumAddress) -> Self {
        value.address().into()
    }
}

impl From<&EthereumAddress> for StoredTokenAddress {
    fn from(value: &EthereumAddress) -> Self {
        value.address().into()
    }
}

impl From<SolanaAddress> for StoredTokenAddress {
    fn from(value: SolanaAddress) -> Self {
        value.pubkey().into()
    }
}

impl From<&SolanaAddress> for StoredTokenAddress {
    fn from(value: &SolanaAddress) -> Self {
        value.pubkey().into()
    }
}

#[derive(Debug, thiserror::Error)]
pub struct TokenAddressParseError;

impl fmt::Display for TokenAddressParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid token address")
    }
}

impl FromStr for StoredTokenAddress {
    type Err = TokenAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match (Pubkey::from_str(s), H160::from_str(s)) {
            (Ok(pubkey), _) => Ok(Self::Solana(pubkey)),
            (_, Ok(ethereum)) => Ok(Self::Ethereum(ethereum)),
            _ => Err(TokenAddressParseError),
        }
    }
}

impl From<Pubkey> for StoredTokenAddress {
    fn from(value: Pubkey) -> Self {
        Self::Solana(value)
    }
}

impl From<H160> for StoredTokenAddress {
    fn from(value: H160) -> Self {
        Self::Ethereum(value)
    }
}

impl<DB> Type<DB> for StoredTokenAddress
where
    DB: Database,
    String: Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <String as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <String as Type<DB>>::compatible(ty)
    }
}

impl<'q, DB> Encode<'q, DB> for StoredTokenAddress
where
    DB: Database,
    String: Encode<'q, DB>,
{
    fn encode_by_ref(&self, buf: &mut <DB as HasArguments<'q>>::ArgumentBuffer) -> IsNull {
        <String as Encode<DB>>::encode(self.to_string(), buf)
    }
}

impl<'r, DB> Decode<'r, DB> for StoredTokenAddress
where
    DB: Database,
    String: Decode<'r, DB>,
{
    fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
        let s = <String as Decode<DB>>::decode(value)?;
        let token_address = StoredTokenAddress::from_str(s.as_str()).map_err(|e| Box::new(e) as BoxDynError)?;
        Ok(token_address)
    }
}

impl FromRow<'_, PgRow> for StoredTokenAddress {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        row.try_get(0)
    }
}

impl<DB> Type<DB> for TokenAddress
where
    DB: Database,
    String: Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <StoredTokenAddress as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <StoredTokenAddress as Type<DB>>::compatible(ty)
    }
}

impl<'r, DB> Decode<'r, DB> for TokenAddress
where
    DB: Database,
    String: Decode<'r, DB>,
{
    fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
        let s = <String as Decode<DB>>::decode(value)?;
        let token_address = StoredTokenAddress::from_str(s.as_str()).map_err(|e| Box::new(e) as BoxDynError)?;
        Ok(token_address.into())
    }
}

impl FromRow<'_, PgRow> for TokenAddress {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        row.try_get(0)
    }
}
