use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

pub struct Base58<T>(T);

impl<T: AsRef<[u8]>> Display for Base58<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        bs58::encode(self.0.as_ref()).into_string().fmt(f)
    }
}

impl<T: AsRef<[u8]>> FromStr for Base58<T> {
    type Err = bs58::decode::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        bs58::decode(s).into_vec().map(Base58)
    }
}

impl<T: AsRef<[u8]>> Deserialize for Base58<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer,
    {
        let bytes = String::deserialize(deserializer)?;
        let bytes = Base58::from_str(&bytes).map_err(serde::de::Error::custom)?;
        Ok(bytes)
    }
}

impl<T: AsRef<[u8]>> Serialize for Base58<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::Base58;
    use sqlx::{
        database::{HasArguments, HasValueRef},
        encode::IsNull,
        error::BoxDynError,
        Database, Decode, Encode, Type,
    };
    use std::str::FromStr;

    impl<T: AsRef<[u8]>, DB: Database> Type<DB> for Base58<T> {
        fn type_info() -> DBType {
            <String as Type<DB>>::type_info()
        }
    }

    impl<'q, T: AsRef<[u8]>, DB: Database> Encode<'q, DB> for Base58<T> {
        fn encode_by_ref(&self, buf: &mut <DB as HasArguments<'q>>::ArgumentBuffer) -> IsNull {
            <String as Encode<DB>>::encode(self.to_string(), buf)
        }
    }

    impl<'r, T: AsRef<[u8]>, DB: Database> Decode<'r, DB> for Base58<T> {
        fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
            let s = <String as Decode<DB>>::decode(value)?;
            let bytes = Base58::from_str(&s).map_err(|e| Box::new(e) as BoxDynError)?;
            Ok(bytes)
        }
    }
}
