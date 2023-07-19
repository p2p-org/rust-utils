use jsonrpsee::core::Cow;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, Same, SerializeAs};
use std::{
    convert::Infallible,
    fmt::{Display, Formatter},
    ops::{Deref, DerefMut},
    str::FromStr,
};

#[serde_with::serde_as]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Hash, PartialOrd, Ord)]
#[serde(transparent)]
pub struct AsString<T>(#[serde_as(as = "serde_with::DisplayFromStr")] pub T)
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display;

impl<T> Deref for AsString<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for AsString<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub struct Base58<T = Same>(pub T);

impl<T> Base58<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
    pub fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> From<T> for Base58<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for Base58<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Base58<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: AsRef<[u8]>> Display for Base58<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        bs58::encode(self.0.as_ref()).into_string().fmt(f)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Base58Error<T> {
    #[error("{0}")]
    Error(T),
    #[error("base58 decode error: {0}")]
    Decode(#[from] bs58::decode::Error),
}

impl<T, E> FromStr for Base58<T>
where
    Base58<T>: for<'a> TryFrom<&'a [u8], Error = E>,
{
    type Err = Base58Error<E>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = bs58::decode(s).into_vec()?;
        (&*bytes).try_into().map_err(Base58Error::Error)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid slice size {0}, expected {1}")]
pub struct WrongSliceSize(usize, usize);

impl<'a, const N: usize> TryFrom<&'a [u8]> for Base58<[u8; N]> {
    type Error = WrongSliceSize;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() != N {
            return Err(WrongSliceSize(value.len(), N));
        }
        let mut buf = [0; N];
        buf[..].clone_from_slice(value);
        Ok(Self(buf))
    }
}

impl<'a> TryFrom<&'a [u8]> for Base58<Vec<u8>> {
    type Error = Infallible;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(Base58(value.into()))
    }
}

impl<'a, 'de: 'a, T> Deserialize<'de> for Base58<T>
where
    Base58<T>: FromStr,
    <Base58<T> as FromStr>::Err: Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Cow::<'de, str>::deserialize(deserializer)?;
        let bytes = Base58::from_str(&*bytes).map_err(serde::de::Error::custom)?;
        Ok(bytes)
    }
}

impl<T: AsRef<[u8]>> Serialize for Base58<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for Base58
where
    Base58<T>: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Base58::deserialize(deserializer)?.0)
    }
}

impl<T: AsRef<[u8]>> SerializeAs<T> for Base58 {
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&bs58::encode(source).into_string())
    }
}

#[cfg(feature = "db")]
mod db {
    use super::{AsString, Base58};
    use sqlx::{
        database::{HasArguments, HasValueRef},
        encode::IsNull,
        error::BoxDynError,
        Database, Decode, Encode, Type,
    };
    use std::{fmt::Display, str::FromStr};

    impl<T, DB> Type<DB> for AsString<T>
    where
        T: Display + FromStr,
        DB: Database,
        <T as FromStr>::Err: Display,
        String: Type<DB>,
    {
        fn type_info() -> DB::TypeInfo {
            <String as Type<DB>>::type_info()
        }

        fn compatible(ty: &DB::TypeInfo) -> bool {
            <String as Type<DB>>::compatible(ty)
        }
    }

    impl<'q, T, DB> Encode<'q, DB> for AsString<T>
    where
        T: Display + FromStr,
        DB: Database,
        <T as FromStr>::Err: Display,
        String: Encode<'q, DB>,
    {
        fn encode_by_ref(&self, buf: &mut <DB as HasArguments<'q>>::ArgumentBuffer) -> IsNull {
            <String as Encode<DB>>::encode(self.0.to_string(), buf)
        }
    }

    impl<'r, T, DB> Decode<'r, DB> for AsString<T>
    where
        T: Display + FromStr,
        DB: Database,
        <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
        String: Decode<'r, DB>,
    {
        fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
            let s = <String as Decode<DB>>::decode(value)?;
            let bytes = T::from_str(&s).map_err(|e| Box::new(e) as BoxDynError)?;
            Ok(AsString(bytes))
        }
    }

    impl<T, DB> Type<DB> for Base58<T>
    where
        T: AsRef<[u8]>,
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

    impl<'q, T, DB> Encode<'q, DB> for Base58<T>
    where
        T: AsRef<[u8]>,
        DB: Database,
        String: Encode<'q, DB>,
    {
        fn encode_by_ref(&self, buf: &mut <DB as HasArguments<'q>>::ArgumentBuffer) -> IsNull {
            <String as Encode<DB>>::encode(self.to_string(), buf)
        }
    }

    impl<'r, T, DB> Decode<'r, DB> for Base58<T>
    where
        Base58<T>: FromStr,
        <Base58<T> as FromStr>::Err: std::error::Error + Send + Sync + 'static,
        DB: Database,
        String: Decode<'r, DB>,
    {
        fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
            let s = <String as Decode<DB>>::decode(value)?;
            let bytes = Base58::from_str(&s).map_err(|e| Box::new(e) as BoxDynError)?;
            Ok(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Base58;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;

    #[test]
    fn base58_serde_as() {
        #[serde_as]
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Data {
            #[serde_as(as = "Base58")]
            value: Vec<u8>,
        }

        let data = Data {
            value: vec![1, 2, 3, 4, 5],
        };
        let json = serde_json::to_string(&data).unwrap();
        let data1 = serde_json::from_str(&json).unwrap();
        assert_eq!(data, data1);
    }

    #[test]
    fn base58_serde() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Data {
            value: Base58<Vec<u8>>,
        }

        let data = Data {
            value: Base58(vec![1, 2, 3, 4, 5]),
        };
        let json = serde_json::to_string(&data).unwrap();
        let data1 = serde_json::from_str(&json).unwrap();
        assert_eq!(data, data1);
    }
}
