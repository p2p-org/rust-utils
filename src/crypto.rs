use borsh::BorshSerialize;
use ed25519_dalek::{Keypair, PublicKey, Signature, SignatureError, Signer, Verifier, PUBLIC_KEY_LENGTH};

pub trait KeypairExt {
    fn new_rand() -> Self;
    fn sign_borsh<M: borsh::BorshSerialize>(&self, message: &M) -> Signature;
}

pub trait PublicKeyExt {
    fn new_zeroed() -> Self;
    fn to_base58(&self) -> String;
    fn verify_borsh<M: borsh::BorshSerialize>(&self, message: &M, signature: &Signature) -> Result<(), SignatureError>;
}

impl KeypairExt for Keypair {
    fn new_rand() -> Self {
        let mut rng = rand::thread_rng();
        Keypair::generate(&mut rng)
    }

    fn sign_borsh<M: BorshSerialize>(&self, message: &M) -> Signature {
        let message = borsh::to_vec(message).expect("message must be serializable");
        self.sign(&message)
    }
}

impl PublicKeyExt for PublicKey {
    fn new_zeroed() -> Self {
        PublicKey::from_bytes(&[0; PUBLIC_KEY_LENGTH]).unwrap()
    }

    fn to_base58(&self) -> String {
        bs58::encode(self).into_string()
    }

    fn verify_borsh<M: BorshSerialize>(&self, message: &M, signature: &Signature) -> Result<(), SignatureError> {
        let message = borsh::to_vec(message).expect("message must be serializable");
        self.verify(&message, signature)
    }
}

#[cfg(feature = "solana-sdk")]
mod solana {
    use super::{KeypairExt, PublicKeyExt};
    use borsh::BorshSerialize;
    use ed25519_dalek::{PublicKey, Signature, SignatureError};
    use solana_sdk::{
        pubkey::Pubkey,
        signer::{keypair::Keypair, Signer},
    };

    impl KeypairExt for Keypair {
        fn new_rand() -> Self {
            Keypair::new()
        }

        fn sign_borsh<M: BorshSerialize>(&self, message: &M) -> Signature {
            let message = borsh::to_vec(message).expect("message must be serializable");
            let signature = self.sign_message(&message);
            Signature::from_bytes(signature.as_ref()).expect("invalid signature")
        }
    }

    impl PublicKeyExt for Pubkey {
        fn new_zeroed() -> Self {
            Pubkey::default()
        }

        fn to_base58(&self) -> String {
            self.to_string()
        }

        fn verify_borsh<M: BorshSerialize>(&self, message: &M, signature: &Signature) -> Result<(), SignatureError> {
            let pubkey = PublicKey::from_bytes(self.as_ref())?;
            pubkey.verify_borsh(message, signature)
        }
    }
}

#[cfg(all(test))]
mod tests {
    use super::*;

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
