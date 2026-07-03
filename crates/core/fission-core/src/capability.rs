use crate::data_stream::{
    BoxFissionDataStream, DataStreamId, DataStreamRegistry, FissionDataStreamError,
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Context passed to an operation capability provider.
#[derive(Clone, Debug)]
pub struct CapabilityCtx {
    /// Request id that identifies the corresponding effect envelope.
    pub req_id: u64,
    data_streams: DataStreamRegistry,
}

impl CapabilityCtx {
    #[doc(hidden)]
    pub fn new_runtime(req_id: u64, data_streams: DataStreamRegistry) -> Self {
        Self {
            req_id,
            data_streams,
        }
    }

    /// Registers a host-owned data stream and returns the handle that can be
    /// passed back through a capability result.
    pub fn register_data_stream(&self, stream: BoxFissionDataStream) -> DataStreamId {
        self.data_streams.register(stream)
    }

    /// Opens a runtime-owned data stream for capability implementations that
    /// need to consume stream handles supplied by app code.
    pub fn open_data_stream(
        &self,
        id: DataStreamId,
    ) -> Result<BoxFissionDataStream, FissionDataStreamError> {
        self.data_streams.open(id)
    }

    /// Releases a previously registered stream without consuming it.
    pub fn release_data_stream(&self, id: DataStreamId) -> bool {
        self.data_streams.release(id)
    }
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

pub const OPEN_URL: CapabilityType<OpenUrlCapability> = CapabilityType::new("fission.ui.open_url");

/// Generic request for opening one or more local/user-granted files.
///
/// The contract is intentionally portable:
/// - no raw local paths are exposed,
/// - the shell chooses the native picker UI,
/// - and selected files are returned as stream handles plus metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PickOpenFilesRequest {
    pub allow_multiple: bool,
    pub mime_types: Vec<String>,
    pub extensions: Vec<String>,
}

/// A user-granted file returned from a picker capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PickedFile {
    /// User-facing file name supplied by the host.
    pub name: String,
    /// MIME type when the host can identify it.
    pub content_type: Option<String>,
    /// Total byte length when the host can determine it up front.
    pub byte_len: Option<u64>,
    /// Runtime-owned stream containing the selected file contents.
    pub stream: DataStreamId,
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
                content_type: Some("application/pdf".into()),
                byte_len: Some(5),
                stream: DataStreamId(7),
            }],
        };
        let bytes = serde_json::to_vec(&result).unwrap();
        let decoded: PickOpenFilesResult = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, result);
    }

    #[test]
    fn open_url_round_trips() {
        let request = OpenUrlRequest {
            url: "https://fission.dev".into(),
            in_app: false,
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: OpenUrlRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }
}
