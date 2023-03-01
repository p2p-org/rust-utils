use ethereum_types::Address;
use rustc_hex::FromHexError;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

use std::{fmt, str::FromStr};

#[derive(PartialEq, Eq, Clone, sqlx::Type, SerializeDisplay, DeserializeFromStr)]
#[sqlx(transparent)]
pub struct EthereumAddress(String);

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ParseAddressError {
    #[error("String is the wrong size")]
    WrongSize,
    #[error("Invalid Address string")]
    Invalid,
}

impl FromStr for EthereumAddress {
    type Err = ParseAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Address::from_str(s) {
            Ok(_) => Ok(EthereumAddress(s.to_owned())),
            Err(err) if matches!(err, FromHexError::InvalidHexCharacter { .. }) => Err(ParseAddressError::Invalid),
            Err(err) if matches!(err, FromHexError::InvalidHexLength) => Err(ParseAddressError::WrongSize),
            _ => unreachable!(),
        }
    }
}

impl TryFrom<&str> for EthereumAddress {
    type Error = ParseAddressError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        EthereumAddress::from_str(s)
    }
}

impl EthereumAddress {
    pub fn new(address_vec: &[u8]) -> Self {
        Self(format!("{:#x}", Address::from_slice(address_vec)))
    }

    pub fn new_as_string(s: String) -> Self {
        Self(s)
    }

    pub fn new_rand() -> Self {
        EthereumAddress(format!("{:#x}", Address::random()))
    }
}

impl AsRef<str> for EthereumAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<EthereumAddress> for String {
    fn from(address: EthereumAddress) -> Self {
        address.0
    }
}

impl fmt::Debug for EthereumAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl fmt::Display for EthereumAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}
