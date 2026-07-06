//! Ed25519 request signing.
//!
//! Binance supports Ed25519 API keys. Ed25519 signing is roughly an order of
//! magnitude faster than HMAC-SHA256 and produces a fixed 64-byte signature, so
//! it is preferred on the order hot path. The signature is computed over the
//! request's query string and transmitted base64-encoded.
//!
//! Only the seed (private key) is held in memory; it is loaded from the
//! encrypted-at-rest key material by the engine and never logged.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};

use exchange_common::ExchangeError;

/// An Ed25519 signer over a 32-byte seed.
pub struct Ed25519Signer {
    key: SigningKey,
}

impl Ed25519Signer {
    /// Builds a signer from a 32-byte seed.
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Ed25519Signer {
        Ed25519Signer {
            key: SigningKey::from_bytes(&seed),
        }
    }

    /// Builds a signer from a seed slice, which must be exactly 32 bytes.
    pub fn from_seed_slice(seed: &[u8]) -> Result<Ed25519Signer, ExchangeError> {
        let arr: [u8; 32] = seed
            .try_into()
            .map_err(|_| ExchangeError::Parse("ed25519 seed must be 32 bytes".to_owned()))?;
        Ok(Ed25519Signer::from_seed(arr))
    }

    /// Signs a request query string, returning the base64-encoded signature to
    /// append as the `signature` parameter.
    #[must_use]
    pub fn sign_query(&self, query: &str) -> String {
        let sig = self.key.sign(query.as_bytes());
        STANDARD.encode(sig.to_bytes())
    }

    /// The raw public (verifying) key bytes.
    #[must_use]
    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// The base64-encoded public key (what Binance registers for the API key).
    #[must_use]
    pub fn verifying_key_base64(&self) -> String {
        STANDARD.encode(self.verifying_key_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    const SEED: [u8; 32] = [7u8; 32];

    #[test]
    fn signature_verifies() {
        let signer = Ed25519Signer::from_seed(SEED);
        let query =
            "symbol=BTCUSDT&side=BUY&type=LIMIT&quantity=0.001&price=60000&timestamp=1700000000000";
        let sig_b64 = signer.sign_query(query);

        let sig_bytes = STANDARD.decode(&sig_b64).unwrap();
        let sig = Signature::from_slice(&sig_bytes).unwrap();
        let vk = VerifyingKey::from_bytes(&signer.verifying_key_bytes()).unwrap();
        assert!(vk.verify(query.as_bytes(), &sig).is_ok());
    }

    #[test]
    fn signing_is_deterministic() {
        let signer = Ed25519Signer::from_seed(SEED);
        let q = "timestamp=1700000000000";
        assert_eq!(signer.sign_query(q), signer.sign_query(q));
    }

    #[test]
    fn tampered_message_fails_verification() {
        let signer = Ed25519Signer::from_seed(SEED);
        let sig_b64 = signer.sign_query("timestamp=1700000000000");
        let sig = Signature::from_slice(&STANDARD.decode(&sig_b64).unwrap()).unwrap();
        let vk = VerifyingKey::from_bytes(&signer.verifying_key_bytes()).unwrap();
        assert!(vk.verify(b"timestamp=1700000000001", &sig).is_err());
    }

    #[test]
    fn signature_is_64_bytes_base64() {
        let signer = Ed25519Signer::from_seed(SEED);
        let sig_b64 = signer.sign_query("x=1");
        assert_eq!(STANDARD.decode(&sig_b64).unwrap().len(), 64);
    }

    #[test]
    fn seed_slice_length_checked() {
        assert!(Ed25519Signer::from_seed_slice(&[0u8; 16]).is_err());
        assert!(Ed25519Signer::from_seed_slice(&[0u8; 32]).is_ok());
    }
}
