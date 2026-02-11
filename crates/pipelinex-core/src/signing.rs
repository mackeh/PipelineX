use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A signed report envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedReport {
    pub payload: String,
    pub signature: String,
    pub public_key: String,
    pub algorithm: String,
}

/// Generate an Ed25519 keypair as PEM-like hex strings.
pub fn generate_keypair() -> Result<(String, String)> {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let private_hex = hex::encode(signing_key.to_bytes());
    let public_hex = hex::encode(verifying_key.to_bytes());

    Ok((private_hex, public_hex))
}

/// Sign a JSON payload with an Ed25519 private key (hex-encoded).
pub fn sign_report(payload: &str, private_key_hex: &str) -> Result<SignedReport> {
    use ed25519_dalek::{Signer, SigningKey};

    let key_bytes = hex::decode(private_key_hex).context("Invalid private key hex")?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Private key must be 32 bytes"))?;

    let signing_key = SigningKey::from_bytes(&key_array);
    let signature = signing_key.sign(payload.as_bytes());
    let public_hex = hex::encode(signing_key.verifying_key().to_bytes());

    Ok(SignedReport {
        payload: payload.to_string(),
        signature: hex::encode(signature.to_bytes()),
        public_key: public_hex,
        algorithm: "Ed25519".to_string(),
    })
}

/// Verify a signed report with a public key (hex-encoded).
pub fn verify_report(report: &SignedReport, public_key_hex: &str) -> Result<bool> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let key_bytes = hex::decode(public_key_hex).context("Invalid public key hex")?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Public key must be 32 bytes"))?;

    let verifying_key =
        VerifyingKey::from_bytes(&key_array).context("Invalid Ed25519 public key")?;

    let sig_bytes = hex::decode(&report.signature).context("Invalid signature hex")?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Signature must be 64 bytes"))?;

    let signature = Signature::from_bytes(&sig_array);

    match verifying_key.verify(report.payload.as_bytes(), &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keypair() {
        let (private_key, public_key) = generate_keypair().unwrap();
        assert_eq!(private_key.len(), 64); // 32 bytes * 2 hex chars
        assert_eq!(public_key.len(), 64);
    }

    #[test]
    fn test_sign_and_verify() {
        let (private_key, public_key) = generate_keypair().unwrap();
        let payload = r#"{"findings": [], "score": 95}"#;

        let signed = sign_report(payload, &private_key).unwrap();
        assert_eq!(signed.algorithm, "Ed25519");
        assert!(!signed.signature.is_empty());

        let valid = verify_report(&signed, &public_key).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_verify_tampered() {
        let (private_key, public_key) = generate_keypair().unwrap();
        let payload = r#"{"findings": [], "score": 95}"#;

        let mut signed = sign_report(payload, &private_key).unwrap();
        signed.payload = r#"{"findings": [], "score": 100}"#.to_string(); // tampered

        let valid = verify_report(&signed, &public_key).unwrap();
        assert!(!valid);
    }

    #[test]
    fn test_verify_wrong_key() {
        let (private_key, _) = generate_keypair().unwrap();
        let (_, other_public) = generate_keypair().unwrap();

        let signed = sign_report("test", &private_key).unwrap();
        let valid = verify_report(&signed, &other_public).unwrap();
        assert!(!valid);
    }
}
