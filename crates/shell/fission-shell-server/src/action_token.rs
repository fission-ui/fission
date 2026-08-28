use anyhow::{bail, Context, Result};
use base64::prelude::{Engine as _, BASE64_URL_SAFE_NO_PAD};
use fission_core::{Action, ActionEnvelope};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// A serialized application action authenticated for one SSR route and node.
pub struct SignedServerAction {
    /// Route against which the action was issued.
    pub route_path: String,
    /// Stable widget node that should receive the action.
    pub target_node: u128,
    /// Typed action identifier and encoded payload to dispatch.
    pub action: ActionEnvelope,
    /// Expiry time as Unix seconds.
    pub expires_unix: u64,
    /// Per-token replay identifier covered by the signature.
    pub nonce: String,
    /// Keyed BLAKE3 signature encoded as hexadecimal.
    pub signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Trusted action data returned after token verification.
pub struct VerifiedServerAction {
    /// Verified route path.
    pub route_path: String,
    /// Verified target widget node.
    pub target_node: u128,
    /// Verified action envelope.
    pub action: ActionEnvelope,
}

#[derive(Clone)]
/// Signs and verifies expiring server-action tokens.
///
/// A signer also tracks consumed nonces for `verify_once`, preventing a valid
/// mutation token from being replayed within this process.
pub struct ServerActionSigner {
    key: [u8; 32],
    used_tokens: Arc<Mutex<BTreeMap<String, u64>>>,
}

const MAX_REPLAY_CACHE_TOKENS: usize = 100_000;

impl ServerActionSigner {
    /// Creates a signer by deriving a fixed-size key from `secret`.
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        let hash = blake3::hash(secret.as_ref());
        Self {
            key: *hash.as_bytes(),
            used_tokens: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Creates a predictable development signer.
    ///
    /// This key is public and must not be used for a deployed application.
    pub fn development() -> Self {
        Self::new(b"fission-development-server-action-key")
    }

    /// Signs a typed action for a route and target node for the supplied TTL.
    pub fn sign<A: Action>(
        &self,
        route_path: impl Into<String>,
        target_node: u128,
        action: A,
        ttl: Duration,
    ) -> SignedServerAction {
        self.sign_envelope(route_path, target_node, action.into(), ttl)
    }

    /// Signs an existing action envelope without decoding its payload.
    pub fn sign_envelope(
        &self,
        route_path: impl Into<String>,
        target_node: u128,
        action: ActionEnvelope,
        ttl: Duration,
    ) -> SignedServerAction {
        let route_path = route_path.into();
        let expires_unix = unix_now().saturating_add(ttl.as_secs());
        let nonce = nonce_for(&route_path, target_node, &action, expires_unix);
        let signature = self.signature(&route_path, target_node, &action, expires_unix, &nonce);
        SignedServerAction {
            route_path,
            target_node,
            action,
            expires_unix,
            nonce,
            signature,
        }
    }

    /// Verifies signature and expiry without consuming the token.
    pub fn verify(&self, token: &SignedServerAction) -> Result<VerifiedServerAction> {
        if token.expires_unix < unix_now() {
            bail!("server action token expired");
        }
        let expected = self.signature(
            &token.route_path,
            token.target_node,
            &token.action,
            token.expires_unix,
            &token.nonce,
        );
        if !constant_time_eq(expected.as_bytes(), token.signature.as_bytes()) {
            bail!("server action token signature mismatch");
        }
        Ok(VerifiedServerAction {
            route_path: token.route_path.clone(),
            target_node: token.target_node,
            action: token.action.clone(),
        })
    }

    /// Verifies and consumes a token, rejecting subsequent replay attempts.
    pub fn verify_once(&self, token: &SignedServerAction) -> Result<VerifiedServerAction> {
        let verified = self.verify(token)?;
        let replay_key = replay_key(token);
        let mut used = self
            .used_tokens
            .lock()
            .map_err(|_| anyhow::anyhow!("server action token replay cache lock poisoned"))?;
        let now = unix_now();
        used.retain(|_, expires_unix| *expires_unix >= now);
        if used.contains_key(&replay_key) {
            bail!("server action token replayed");
        }
        if used.len() >= MAX_REPLAY_CACHE_TOKENS {
            bail!("server action token replay cache is full");
        }
        used.insert(replay_key, token.expires_unix);
        Ok(verified)
    }

    /// Encodes a token as URL-safe, unpadded base64 JSON.
    pub fn encode(&self, token: &SignedServerAction) -> Result<String> {
        let bytes = serde_json::to_vec(token).context("failed to encode server action token")?;
        Ok(BASE64_URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Decodes a token; call `verify` or `verify_once` before trusting it.
    pub fn decode(&self, encoded: &str) -> Result<SignedServerAction> {
        let bytes = BASE64_URL_SAFE_NO_PAD
            .decode(encoded)
            .context("failed to decode server action token")?;
        serde_json::from_slice(&bytes).context("failed to parse server action token")
    }

    fn signature(
        &self,
        route_path: &str,
        target_node: u128,
        action: &ActionEnvelope,
        expires_unix: u64,
        nonce: &str,
    ) -> String {
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(b"fission.server.action.v1");
        hasher.update(route_path.as_bytes());
        hasher.update(&target_node.to_le_bytes());
        hasher.update(&action.id.as_u128().to_le_bytes());
        hasher.update(&(action.payload.len() as u64).to_le_bytes());
        hasher.update(&action.payload);
        hasher.update(&expires_unix.to_le_bytes());
        hasher.update(nonce.as_bytes());
        to_hex(hasher.finalize().as_bytes())
    }
}

fn replay_key(token: &SignedServerAction) -> String {
    format!(
        "{}:{}:{}:{}",
        token.route_path, token.target_node, token.nonce, token.signature
    )
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn nonce_for(
    route_path: &str,
    target_node: u128,
    action: &ActionEnvelope,
    expires_unix: u64,
) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"fission.server.action.nonce.v1");
    hasher.update(route_path.as_bytes());
    hasher.update(&target_node.to_le_bytes());
    hasher.update(&action.id.as_u128().to_le_bytes());
    hasher.update(&action.payload);
    hasher.update(&expires_unix.to_le_bytes());
    hasher.update(&now.as_nanos().to_le_bytes());
    hasher.update(&std::process::id().to_le_bytes());
    to_hex(&hasher.finalize().as_bytes()[..16])
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::{Action, ActionId};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AddToCart {
        sku: String,
    }

    impl Action for AddToCart {
        fn static_id() -> ActionId {
            ActionId::from_name("test::AddToCart")
        }
    }

    #[test]
    fn signed_action_tokens_round_trip_and_reject_tampering() {
        let signer = ServerActionSigner::new("secret");
        let token = signer.sign(
            "/",
            7,
            AddToCart { sku: "abc".into() },
            Duration::from_secs(60),
        );
        let encoded = signer.encode(&token).unwrap();
        let decoded = signer.decode(&encoded).unwrap();
        assert_eq!(signer.verify(&decoded).unwrap().target_node, 7);

        let mut tampered = decoded;
        tampered.target_node = 8;
        assert!(signer.verify(&tampered).is_err());
    }

    #[test]
    fn signed_action_tokens_reject_expiry_wrong_secret_and_payload_tampering() {
        let signer = ServerActionSigner::new("secret");
        let token = signer.sign(
            "/cart",
            7,
            AddToCart { sku: "abc".into() },
            Duration::from_secs(60),
        );

        let mut expired = token.clone();
        expired.expires_unix = 0;
        assert!(signer.verify(&expired).is_err());

        let other_signer = ServerActionSigner::new("different-secret");
        assert!(other_signer.verify(&token).is_err());

        let mut tampered = token;
        tampered.action.payload = serde_json::to_vec(&AddToCart { sku: "xyz".into() }).unwrap();
        assert!(signer.verify(&tampered).is_err());
    }

    #[test]
    fn signed_action_tokens_are_single_use_when_verified_once() {
        let signer = ServerActionSigner::new("secret");
        let token = signer.sign(
            "/cart",
            7,
            AddToCart { sku: "abc".into() },
            Duration::from_secs(60),
        );

        assert!(signer.verify_once(&token).is_ok());
        assert!(signer.verify_once(&token).is_err());
        assert!(signer.verify(&token).is_ok());
    }

    #[test]
    fn signed_action_token_decode_rejects_malformed_base64_and_json() {
        let signer = ServerActionSigner::new("secret");
        assert!(signer.decode("not base64!").is_err());

        let not_json = BASE64_URL_SAFE_NO_PAD.encode(b"not-json");
        assert!(signer.decode(&not_json).is_err());
    }
}
