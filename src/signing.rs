use crate::canonical::{canonical_hash, remove_field};
use crate::event::event_hash;
use anyhow::{Context, Result, bail};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalKeyFile {
    pub schema: String,
    pub algorithm: String,
    pub key_id: String,
    pub public_key: String,
    pub secret_key: String,
}

pub fn generate_key() -> LocalKeyFile {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    LocalKeyFile {
        schema: "agentprov.dev/local-key/v1".to_owned(),
        algorithm: "ed25519".to_owned(),
        key_id: format!("key_{}", Uuid::new_v4().simple()),
        public_key: hex::encode(verifying_key.to_bytes()),
        secret_key: hex::encode(signing_key.to_bytes()),
    }
}

pub fn public_key_view(key: &LocalKeyFile) -> Value {
    json!({
        "schema": "agentprov.dev/public-key/v1",
        "algorithm": key.algorithm,
        "key_id": key.key_id,
        "public_key": key.public_key,
    })
}

pub fn inspect_key_view(key: &LocalKeyFile) -> Value {
    json!({
        "schema": key.schema,
        "algorithm": key.algorithm,
        "key_id": key.key_id,
        "public_key": key.public_key,
        "has_secret_key": !key.secret_key.is_empty(),
    })
}

pub fn read_key(path: &Path) -> Result<LocalKeyFile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("read key file {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parse key file {}", path.display()))
}

fn signing_key(key: &LocalKeyFile) -> Result<SigningKey> {
    let bytes = hex::decode(&key.secret_key).context("decode secret key hex")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("secret key must be 32 bytes"))?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn verifying_key(public_key: &str) -> Result<VerifyingKey> {
    let bytes = hex::decode(public_key).context("decode public key hex")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("public key must be 32 bytes"))?;
    VerifyingKey::from_bytes(&bytes).context("parse verifying key")
}

pub fn sign_value(value: &mut Value, key: &LocalKeyFile) -> Result<()> {
    remove_field(value, "signature");
    value["key_id"] = Value::String(key.key_id.clone());
    let hash = signed_payload_hash(value)?;
    if value.get("event_hash").is_some() {
        value["event_hash"] = Value::String(hash.clone());
    }
    let signature = signing_key(key)?.sign(hash.as_bytes());
    value["signature"] = json!({
        "algorithm": "ed25519",
        "key_id": key.key_id,
        "public_key": key.public_key,
        "signature": hex::encode(signature.to_bytes()),
        "signed_hash": hash,
    });
    Ok(())
}

pub fn signed_payload_hash(value: &Value) -> Result<String> {
    if value.get("event_hash").is_some() {
        event_hash(value)
    } else {
        let mut unsigned = value.clone();
        remove_field(&mut unsigned, "signature");
        canonical_hash(&unsigned)
    }
}

pub fn verify_signature(value: &Value) -> Result<()> {
    let signature_value = value
        .get("signature")
        .filter(|value| !value.is_null())
        .context("signature must be present")?;
    let public_key = signature_value
        .get("public_key")
        .and_then(Value::as_str)
        .context("signature.public_key must be present")?;
    let signature_hex = signature_value
        .get("signature")
        .and_then(Value::as_str)
        .context("signature.signature must be present")?;
    let signed_hash = signature_value
        .get("signed_hash")
        .and_then(Value::as_str)
        .context("signature.signed_hash must be present")?;
    let actual_hash = signed_payload_hash(value)?;
    if signed_hash != actual_hash {
        bail!("signed hash mismatch: expected {signed_hash}, actual {actual_hash}");
    }
    let signature_bytes = hex::decode(signature_hex).context("decode signature hex")?;
    let signature = Signature::from_slice(&signature_bytes).context("parse signature")?;
    verifying_key(public_key)?.verify(signed_hash.as_bytes(), &signature)?;
    Ok(())
}
