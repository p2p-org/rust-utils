use std::io;

use serde::{ser::SerializeTupleVariant, Serialize, Serializer};
use strum::AsStaticRef;
use strum_macros::AsStaticStr;
use thiserror::Error;

pub type UtilsResult<T> = Result<T, UtilsError>;

#[derive(Debug, Error, Serialize)]
pub enum FeeTokenProviderError {
    #[error("Duplicate token mint: {0}")]
    DuplicateTokenMint(String),

    #[error("Poison error of {0}")]
    PoisonError(String),
}

#[derive(Debug, Error, AsStaticStr)]
pub enum UtilsError {
    #[error("FeeTokenProvider error: {0}")]
    FeeTokenProviderError(#[from] FeeTokenProviderError),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("JSON serialize error: {0}")]
    JsonSerializeError(#[from] serde_json::Error),
}

impl UtilsError {
    fn as_u32(&self) -> u32 {
        match self {
            Self::IoError(..) => 0,
            Self::JsonSerializeError(..) => 1,
            Self::FeeTokenProviderError(..) => 2,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.as_u32(),
            "message": self.to_string(),
            "data": self,
        })
    }
}

impl Serialize for UtilsError {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let error_type_name = stringify!(ServerError);
        let variant_index = self.as_u32();
        let variant_name = self.as_static();

        match self {
            UtilsError::FeeTokenProviderError(code) => {
                let mut s = ser.serialize_tuple_variant(error_type_name, variant_index, variant_name, 2)?;
                s.serialize_field(match code {
                    FeeTokenProviderError::DuplicateTokenMint(msg) | FeeTokenProviderError::PoisonError(msg) => msg,
                })?;
                s.end()
            },

            UtilsError::JsonSerializeError(error) => {
                let mut s = ser.serialize_tuple_variant(error_type_name, variant_index, variant_name, 1)?;
                s.serialize_field(&format!("{}", error))?;
                s.end()
            },
            UtilsError::IoError(error) => {
                let mut s = ser.serialize_tuple_variant(error_type_name, variant_index, variant_name, 1)?;
                s.serialize_field(&format!("{:?}", error))?;
                s.end()
            },
        }
    }
}
