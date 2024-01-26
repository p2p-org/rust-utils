use std::str::FromStr;

use chrono::Utc;
use ed25519_dalek::{Keypair, PublicKey, Signature, SignatureError, Signer, Verifier, PUBLIC_KEY_LENGTH};
use serde::{Deserialize, Serialize};

pub trait KeypairExt {
    type Signature;
    fn new_rand() -> Self;
    fn sign_slice(&self, message: &[u8]) -> Self::Signature;

    fn sign_borsh<M: borsh::BorshSerialize>(&self, message: &M) -> Self::Signature {
        let message = borsh::to_vec(message).expect("message must be serializable");
        self.sign_slice(&message)
    }
}

pub trait PublicKeyExt<S> {
    fn new_zeroed() -> Self;
    fn from_base58(value: &str) -> Option<Self>
    where
        Self: Sized;

    fn to_base58(&self) -> String;
    fn verify_slice(&self, message: &[u8], signature: &S) -> Result<(), SignatureError>;

    fn verify_borsh<M: borsh::BorshSerialize>(&self, message: &M, signature: &S) -> Result<(), SignatureError> {
        let message = borsh::to_vec(message).expect("message must be serializable");
        self.verify_slice(&message, signature)
    }
}

impl KeypairExt for Keypair {
    type Signature = Signature;

    fn new_rand() -> Self {
        let mut rng = rand::thread_rng();
        Keypair::generate(&mut rng)
    }

    fn sign_slice(&self, message: &[u8]) -> Signature {
        self.sign(message)
    }
}

impl PublicKeyExt<Signature> for PublicKey {
    fn new_zeroed() -> Self {
        PublicKey::from_bytes(&[0; PUBLIC_KEY_LENGTH]).unwrap()
    }
    fn from_base58(value: &str) -> Option<Self> {
        let bytes = bs58::decode(value).into_vec().ok()?;
        PublicKey::from_bytes(&bytes).ok()
    }

    fn to_base58(&self) -> String {
        bs58::encode(self).into_string()
    }

    fn verify_slice(&self, message: &[u8], signature: &Signature) -> Result<(), SignatureError> {
        self.verify(&message, signature)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("signature ttl '{0}' seconds is expired")]
    TTLExpired(u64),
    #[error("wrong signature: {0}")]
    WrongSignature(String),
    #[error("wrong user: {0}")]
    WrongUser(String),
    #[error(transparent)]
    SignatureError(#[from] SignatureError),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimedSignature<T> {
    pub timestamp: u64,
    pub signature: T,
}

impl<T> TimedSignature<T> {
    pub fn new(timestamp: u64, signature: T) -> Self {
        Self { timestamp, signature }
    }
}

pub trait GetSignatureTtl {
    fn get_signature_ttl(&self) -> Option<u64>;
}

pub trait CheckSignature: GetSignatureTtl {
    fn check_signature<T: borsh::ser::BorshSerialize>(
        &self,
        pubkey: &str,
        msg: &T,
        timed_signature: &TimedSignature<&str>,
    ) -> Result<(), Error> {
        if let Some(signature_ttl) = self.get_signature_ttl() {
            // Debug mode if signature_ttl in settings is 0 we have option to skip checking signature
            if signature_ttl == 0 && timed_signature.timestamp == 0 {
                return Ok(());
            }

            if Utc::now().timestamp() as u64 - timed_signature.timestamp > signature_ttl {
                return Err(Error::TTLExpired(signature_ttl));
            }
        }

        let verifying_key = PublicKey::from_base58(pubkey).ok_or(Error::WrongUser(pubkey.to_string()))?;

        let signature = Signature::from_str(timed_signature.signature)
            .map_err(|_| Error::WrongSignature(timed_signature.signature.to_string()))?;

        Ok(verifying_key.verify_borsh(msg, &signature)?)
    }
}

#[cfg(feature = "wrappers")]
mod base58 {
    use crate::wrappers::Base58;
    use ed25519_dalek::{Keypair, PublicKey, Signature, SignatureError};

    impl<'a> TryFrom<&'a [u8]> for Base58<PublicKey> {
        type Error = SignatureError;

        fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
            let pk = PublicKey::from_bytes(value)?;
            Ok(Base58(pk))
        }
    }

    impl<'a> TryFrom<&'a [u8]> for Base58<Keypair> {
        type Error = SignatureError;

        fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
            let pk = Keypair::from_bytes(value)?;
            Ok(Base58(pk))
        }
    }

    impl<'a> TryFrom<&'a [u8]> for Base58<Signature> {
        type Error = SignatureError;

        fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
            let pk = Signature::from_bytes(value)?;
            Ok(Base58(pk))
        }
    }

    #[cfg(feature = "solana")]
    mod solana {
        use crate::wrappers::Base58;
        use ed25519_dalek::SignatureError;
        use solana_sdk::{
            pubkey::Pubkey,
            signature::{Keypair, ParseSignatureError, Signature},
        };
        use std::{array::TryFromSliceError, mem::size_of};

        impl<'a> TryFrom<&'a [u8]> for Base58<Pubkey> {
            type Error = TryFromSliceError;

            fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
                let pk = Pubkey::try_from(value)?;
                Ok(Base58(pk))
            }
        }

        impl<'a> TryFrom<&'a [u8]> for Base58<Keypair> {
            type Error = SignatureError;

            fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
                let pk = Keypair::from_bytes(value)?;
                Ok(Base58(pk))
            }
        }

        impl<'a> TryFrom<&'a [u8]> for Base58<Signature> {
            type Error = ParseSignatureError;

            fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
                if value.len() != size_of::<Signature>() {
                    return Err(ParseSignatureError::WrongSize);
                }

                let signature = Signature::new(value);
                Ok(Base58(signature))
            }
        }
    }
}

#[cfg(feature = "solana")]
mod solana {
    use super::{KeypairExt, PublicKeyExt};
    use ed25519_dalek::SignatureError;
    use solana_sdk::{
        pubkey::Pubkey,
        signature::Signature,
        signer::{keypair::Keypair, Signer},
    };

    impl KeypairExt for Keypair {
        type Signature = Signature;

        fn new_rand() -> Self {
            Keypair::new()
        }

        fn sign_slice(&self, message: &[u8]) -> Self::Signature {
            self.sign_message(message)
        }
    }

    impl PublicKeyExt<Signature> for Pubkey {
        fn new_zeroed() -> Self {
            Pubkey::default()
        }
        fn from_base58(value: &str) -> Option<Self> {
            value.parse().ok()
        }

        fn to_base58(&self) -> String {
            self.to_string()
        }

        fn verify_slice(&self, message: &[u8], signature: &Signature) -> Result<(), SignatureError> {
            if signature.verify(self.as_ref(), &message) {
                Ok(())
            } else {
                Err(SignatureError::new())
            }
        }
    }
}

#[cfg(all(test))]
mod tests {
    use super::*;

    struct TestService;

    impl CheckSignature for TestService {}

    impl GetSignatureTtl for TestService {
        fn get_signature_ttl(&self) -> Option<u64> {
            None
        }
    }

    #[test]
    fn test_signature() {
        let keys = Keypair::new_rand();

        let user = keys.public.to_base58();
        let timestamp = 1234567890;

        let msg = (&user, timestamp);
        let signature = keys.sign_borsh(&msg).to_string();

        let tests_service = TestService;
        tests_service
            .check_signature(&user, &msg, &TimedSignature::new(timestamp, &signature))
            .unwrap();
    }

    #[test]
    fn check_signing() {
        let keypair = Keypair::new_rand();

        let message = "Hello world".to_string();

        let signature = keypair.sign_borsh(&message);

        assert!(keypair.public.verify_borsh(&message, &signature).is_ok());
    }

    #[test]
    fn check_signing_wrong_pubkey() {
        let keypair = Keypair::new_rand();

        let message = "Hello world".to_string();

        let signature = keypair.sign_borsh(&message);

        let second_keypair = Keypair::new_rand();

        assert!(second_keypair.public.verify_borsh(&message, &signature).is_err());
    }

    #[cfg(feature = "solana-sdk")]
    #[test]
    fn check_solana_signing() {
        use solana_sdk::signer::{keypair::Keypair, Signer};
        let keypair = Keypair::new_rand();

        let message = "Hello world".to_string();

        let signature = keypair.sign_borsh(&message);

        assert!(keypair.pubkey().verify_borsh(&message, &signature).is_ok());
    }

    #[cfg(feature = "solana-sdk")]
    #[test]
    fn check_solana_signing_wrong_pubkey() {
        use solana_sdk::signer::{keypair::Keypair, Signer};
        let keypair = Keypair::new_rand();

        let message = "Hello world".to_string();

        let signature = keypair.sign_borsh(&message);

        let second_keypair = Keypair::new_rand();

        assert!(second_keypair.pubkey().verify_borsh(&message, &signature).is_err());
    }
}
