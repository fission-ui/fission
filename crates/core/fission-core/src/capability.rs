use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Context passed to an operation capability provider.
#[derive(Clone, Debug)]
pub struct CapabilityCtx {
    /// Request id that identifies the corresponding effect envelope.
    pub req_id: u64,
}

/// Trait for one-shot host capabilities.
///
/// Capability payload types are fully typed and serialized by the host layer.
/// Callers pass a `CapabilityType<C>` marker plus a typed `C::Request`.
pub trait OperationCapability: Send + 'static {
    type Request: Serialize + for<'de> Deserialize<'de> + Send + 'static;
    type Ok: Serialize + for<'de> Deserialize<'de> + Send + 'static;
    type Err: Serialize + for<'de> Deserialize<'de> + Send + 'static;
}

/// A typed capability identity.
#[derive(Copy, Clone)]
pub struct CapabilityType<C: OperationCapability> {
    /// Capability name used by the shell registry and host providers.
    pub name: &'static str,
    _marker: PhantomData<fn() -> C>,
}

impl<C: OperationCapability> CapabilityType<C> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }
}

impl<C: OperationCapability> std::fmt::Debug for CapabilityType<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilityType")
            .field("name", &self.name)
            .finish()
    }
}

impl<C: OperationCapability> PartialEq for CapabilityType<C> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<C: OperationCapability> Eq for CapabilityType<C> {}

impl<C: OperationCapability> std::hash::Hash for CapabilityType<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationCapabilityInvocation {
    pub capability_name: String,
    pub request: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CapabilityInvocationPayload {
    Operation(OperationCapabilityInvocation),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertRequest {
    pub title: String,
    pub message: String,
}

pub struct AlertCapability;

impl OperationCapability for AlertCapability {
    type Request = AlertRequest;
    type Ok = ();
    type Err = String;
}

pub const SHOW_ALERT: CapabilityType<AlertCapability> =
    CapabilityType::new("fission.ui.alert");

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenUrlRequest {
    pub url: String,
    pub in_app: bool,
}

pub struct OpenUrlCapability;

impl OperationCapability for OpenUrlCapability {
    type Request = OpenUrlRequest;
    type Ok = ();
    type Err = String;
}

pub const OPEN_URL: CapabilityType<OpenUrlCapability> =
    CapabilityType::new("fission.ui.open_url");

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthenticateRequest {
    pub url: String,
    pub callback_scheme: String,
}

pub struct AuthenticateCapability;

impl OperationCapability for AuthenticateCapability {
    type Request = AuthenticateRequest;
    type Ok = ();
    type Err = String;
}

pub const AUTHENTICATE: CapabilityType<AuthenticateCapability> =
    CapabilityType::new("fission.auth.external_session");

/// Generic request for opening one or more local/user-granted files.
///
/// The contract is intentionally portable:
/// - no raw local paths are exposed,
/// - the shell chooses the native picker UI,
/// - and selected files are returned as bytes plus metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PickOpenFilesRequest {
    pub allow_multiple: bool,
    pub mime_types: Vec<String>,
    pub extensions: Vec<String>,
}

/// A user-granted file returned from a picker capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PickedFile {
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// Result payload for a file picker operation.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PickOpenFilesResult {
    pub files: Vec<PickedFile>,
}

/// Error returned by a file picker capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PickOpenFilesError {
    pub code: String,
    pub message: String,
}

pub struct PickOpenFilesCapability;

impl OperationCapability for PickOpenFilesCapability {
    type Request = PickOpenFilesRequest;
    type Ok = PickOpenFilesResult;
    type Err = PickOpenFilesError;
}

pub const PICK_OPEN_FILES: CapabilityType<PickOpenFilesCapability> =
    CapabilityType::new("fission.fs.pick_open");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_open_files_round_trips() {
        let request = PickOpenFilesRequest {
            allow_multiple: true,
            mime_types: vec!["image/png".into(), "application/pdf".into()],
            extensions: vec!["png".into(), "pdf".into()],
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: PickOpenFilesRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);

        let result = PickOpenFilesResult {
            files: vec![PickedFile {
                name: "receipt.pdf".into(),
                content_type: "application/pdf".into(),
                bytes: b"hello".to_vec(),
            }],
        };
        let bytes = serde_json::to_vec(&result).unwrap();
        let decoded: PickOpenFilesResult = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, result);
    }
}
