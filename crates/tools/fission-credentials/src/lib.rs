use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use fission_command_core::DistributionProvider;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct VaultRecord {
    schema_version: u32,
    provider: String,
    created_at_unix_seconds: u64,
    nonce: String,
    ciphertext: String,
}

pub fn provider_secret(
    provider: DistributionProvider,
    env_names: &[&str],
) -> Result<Option<String>> {
    if let Some(name) = env_names.iter().find(|name| env::var_os(name).is_some()) {
        return env::var(name)
            .map(Some)
            .with_context(|| format!("environment variable {name} is not valid UTF-8"));
    }
    let path = vault_record_path(provider)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = load_provider_secret(provider)?;
    String::from_utf8(bytes)
        .map(Some)
        .context("stored provider credential is not valid UTF-8")
}

pub fn read_secret_source(source: &str) -> Result<String> {
    if let Some(name) = source.strip_prefix("env:") {
        env::var(name).with_context(|| format!("environment variable {name} is not set"))
    } else if let Some(path) = source.strip_prefix("file:") {
        fs::read_to_string(path).with_context(|| format!("failed to read credential file {path}"))
    } else {
        bail!("credential source must be env:<NAME> or file:<PATH>")
    }
}

pub fn store_provider_secret(provider: DistributionProvider, secret: &[u8]) -> Result<()> {
    let key = vault_key(true)?;
    let mut nonce = [0u8; 24];
    getrandom::getrandom(&mut nonce)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|error| anyhow::anyhow!("failed to initialize vault cipher: {error}"))?;
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), secret)
        .map_err(|error| anyhow::anyhow!("failed to encrypt credential record: {error}"))?;
    let record = VaultRecord {
        schema_version: 1,
        provider: provider.as_str().to_string(),
        created_at_unix_seconds: now_unix_seconds(),
        nonce: STANDARD_NO_PAD.encode(nonce),
        ciphertext: STANDARD_NO_PAD.encode(ciphertext),
    };
    let path = vault_record_path(provider)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&record)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn load_provider_secret(provider: DistributionProvider) -> Result<Vec<u8>> {
    let path = vault_record_path(provider)?;
    let record: VaultRecord = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
    )?;
    let nonce = STANDARD_NO_PAD
        .decode(record.nonce)
        .context("failed to decode vault nonce")?;
    let ciphertext = STANDARD_NO_PAD
        .decode(record.ciphertext)
        .context("failed to decode vault ciphertext")?;
    let key = vault_key(false)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|error| anyhow::anyhow!("failed to initialize vault cipher: {error}"))?;
    cipher
        .decrypt(XNonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|error| anyhow::anyhow!("failed to decrypt credential record: {error}"))
}

pub fn rotate_provider_secret(provider: DistributionProvider) -> Result<()> {
    let secret = load_provider_secret(provider)?;
    store_provider_secret(provider, &secret)
}

pub fn vault_record_path(provider: DistributionProvider) -> Result<PathBuf> {
    Ok(vault_dir()?.join(format!("{}.json", provider.as_str())))
}

fn vault_key(create: bool) -> Result<[u8; 32]> {
    let entry = keyring::Entry::new("fission", "release-vault")
        .context("failed to open OS credential store for the Fission release vault")?;
    match entry.get_password() {
        Ok(encoded) => decode_vault_key(&encoded),
        Err(error) if create => {
            let mut key = [0u8; 32];
            getrandom::getrandom(&mut key)?;
            entry
                .set_password(&STANDARD_NO_PAD.encode(key))
                .with_context(|| {
                    format!("failed to store Fission vault key in OS credential store: {error}")
                })?;
            Ok(key)
        }
        Err(error) => {
            Err(error).context("Fission vault key does not exist in the OS credential store")
        }
    }
}

fn decode_vault_key(encoded: &str) -> Result<[u8; 32]> {
    let bytes = STANDARD_NO_PAD
        .decode(encoded)
        .context("failed to decode Fission vault key")?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Fission vault key has the wrong length"))?;
    Ok(key)
}

fn vault_dir() -> Result<PathBuf> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .context("HOME/USERPROFILE is not set")?;
    Ok(PathBuf::from(home).join(".fission/vault"))
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
