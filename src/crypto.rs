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
