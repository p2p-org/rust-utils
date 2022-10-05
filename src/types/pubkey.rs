use serde_with::{DeserializeFromStr, SerializeDisplay};
use sqlx::{encode::IsNull, Postgres};
use std::{
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ParseBase58Error {
    #[error("String is the wrong size")]
    WrongSize,
    #[error("Invalid Base58 string")]
    Invalid,
}

/// Maximum string length of a base58 encoded pubkey
const MAX_BASE58_PUBKEY_LEN: usize = 44;
/// Number of bytes in a pubkey
pub const PUBKEY_BYTES: usize = 32;

#[derive(PartialEq, Eq, Clone, SerializeDisplay, DeserializeFromStr)]
pub struct Base58Pubkey {
    pubkey: [u8; PUBKEY_BYTES],
    bs58: String,
}

impl Hash for Base58Pubkey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pubkey.hash(state)
    }
}

impl TryFrom<&str> for Base58Pubkey {
    type Error = ParseBase58Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Base58Pubkey::from_str(s)
    }
}

impl Base58Pubkey {
    pub fn new(pubkey_vec: &[u8]) -> Self {
        let pubkey = <[u8; PUBKEY_BYTES]>::try_from(<&[u8]>::clone(&pubkey_vec))
            .expect("Slice must be the same length as a Pubkey");
        let bs58 = bs58::encode(pubkey).into_string();
        Self { pubkey, bs58 }
    }

    pub fn new_rand() -> Self {
        Base58Pubkey::new(&rand::random::<[u8; PUBKEY_BYTES]>())
    }
}

impl FromStr for Base58Pubkey {
    type Err = ParseBase58Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > MAX_BASE58_PUBKEY_LEN {
            return Err(ParseBase58Error::WrongSize);
        }
        let pubkey_vec = bs58::decode(s).into_vec().map_err(|_| ParseBase58Error::Invalid)?;
        let pubkey: [u8; PUBKEY_BYTES] = pubkey_vec.try_into().map_err(|_| ParseBase58Error::WrongSize)?;
        Ok(Self {
            pubkey,
            bs58: s.to_owned(),
        })
    }
}

impl AsRef<str> for Base58Pubkey {
    fn as_ref(&self) -> &str {
        &self.bs58
    }
}

impl AsRef<[u8]> for Base58Pubkey {
    fn as_ref(&self) -> &[u8] {
        &self.pubkey
    }
}

impl From<Base58Pubkey> for String {
    fn from(key: Base58Pubkey) -> Self {
        key.bs58
    }
}

impl fmt::Debug for Base58Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.bs58)
    }
}

impl fmt::Display for Base58Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.bs58)
    }
}

impl sqlx::Type<Postgres> for Base58Pubkey {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <&str as sqlx::Type<Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <&str as sqlx::Type<Postgres>>::compatible(ty)
    }
}

impl sqlx::Encode<'_, Postgres> for Base58Pubkey {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> IsNull {
        <&str as sqlx::Encode<Postgres>>::encode(self.as_ref(), buf)
    }
}

impl sqlx::Decode<'_, Postgres> for Base58Pubkey {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Base58Pubkey::from_str(<&str as sqlx::Decode<Postgres>>::decode(
            value,
        )?)?)
    }
}
