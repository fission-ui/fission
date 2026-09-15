//! Cross-platform file-system access.
//!
//! Operations can address a path in the host's filesystem namespace or a path
//! resolved from an opaque directory handle. Directory handles are required by
//! hosts such as browsers and document providers that do not expose global OS
//! paths; they are not a framework sandbox for native applications.

use crate::capability::{CapabilityType, OperationCapability};
use crate::DataStreamId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Runtime-owned identity for a directory selected by the user.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirectoryHandleId(pub u64);

/// Access requested for a directory grant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSystemAccessMode {
    /// Read directory entries and file contents.
    #[default]
    Read,
    /// Read and modify entries beneath the selected directory.
    ReadWrite,
}

/// Current host-reported permission for a directory handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSystemPermission {
    Granted,
    Prompt,
    Denied,
    #[default]
    Unknown,
}

/// A directory grant owned by the active shell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryHandle {
    pub id: DirectoryHandleId,
    /// Display name supplied by the host. This is not a path.
    pub name: String,
    pub access: FileSystemAccessMode,
    pub permission: FileSystemPermission,
}

/// A filesystem path interpreted by the active host provider.
///
/// Fission does not normalize, sandbox, or authorize this value. Native hosts
/// pass it to the operating system using the application's actual process
/// permissions. Handle-backed providers interpret it in their own namespace
/// and report a typed error when that namespace cannot represent the path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileSystemPath(String);

impl FileSystemPath {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a child path using the compiling target's path semantics.
    pub fn join(&self, child: impl AsRef<str>) -> Self {
        let mut path = PathBuf::from(&self.0);
        path.push(child.as_ref());
        Self(path.to_string_lossy().into_owned())
    }
}

impl AsRef<str> for FileSystemPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FileSystemPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<String> for FileSystemPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for FileSystemPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// A file-system address.
///
/// `Path` uses the active host's filesystem namespace. `Directory` resolves a
/// provider path from an opaque handle returned by a picker or restoration
/// operation. The latter is the portable option for browser and document-tree
/// providers, while both forms are available to native providers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileSystemLocation {
    Path(FileSystemPath),
    Directory {
        directory: DirectoryHandleId,
        path: FileSystemPath,
    },
}

impl FileSystemLocation {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(FileSystemPath::new(path))
    }

    pub fn directory(directory: DirectoryHandleId, path: impl Into<String>) -> Self {
        Self::Directory {
            directory,
            path: FileSystemPath::new(path),
        }
    }

    pub fn directory_root(directory: DirectoryHandleId) -> Self {
        Self::Directory {
            directory,
            path: FileSystemPath::root(),
        }
    }

    pub fn file_system_path(&self) -> &FileSystemPath {
        match self {
            Self::Path(path) | Self::Directory { path, .. } => path,
        }
    }

    pub fn join(&self, child: impl AsRef<str>) -> Self {
        match self {
            Self::Path(path) => Self::Path(path.join(child)),
            Self::Directory { directory, path } => Self::Directory {
                directory: *directory,
                path: path.join(child),
            },
        }
    }
}

/// Requests a user-visible directory picker.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickDirectoryRequest {
    pub access: FileSystemAccessMode,
    /// Optional app-chosen key used to restore the grant in a future session.
    /// Browser hosts persist only the directory handle, never its contents.
    pub persistence_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickDirectoryResult {
    /// `None` means the user dismissed the picker.
    pub directory: Option<DirectoryHandle>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreDirectoryRequest {
    pub persistence_key: String,
    pub access: FileSystemAccessMode,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreDirectoryResult {
    /// `None` means no persisted grant exists for the key.
    pub directory: Option<DirectoryHandle>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgetDirectoryRequest {
    pub persistence_key: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryPermissionRequest {
    pub directory: DirectoryHandleId,
    pub access: FileSystemAccessMode,
    /// When true, the host may display a permission prompt. Keep this false for
    /// passive status checks that are not initiated by a user gesture.
    pub request: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryPermissionResult {
    pub permission: FileSystemPermission,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSystemEntryKind {
    File,
    Directory,
    #[default]
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemEntry {
    pub name: String,
    pub location: FileSystemLocation,
    pub kind: FileSystemEntryKind,
    pub byte_len: Option<u64>,
    /// Milliseconds since the Unix epoch when the host exposes it.
    pub modified_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDirectoryRequest {
    pub location: FileSystemLocation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDirectoryResult {
    pub entries: Vec<FileSystemEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatEntryRequest {
    pub location: FileSystemLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatEntryResult {
    pub entry: FileSystemEntry,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub location: FileSystemLocation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub stream: DataStreamId,
    pub byte_len: Option<u64>,
    pub modified_millis: Option<u64>,
}

/// Source bytes for a file write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileWriteSource {
    /// Convenient for small generated values such as preferences or documents.
    Bytes(Vec<u8>),
    /// A single-consumer runtime stream, suitable for large or host-produced data.
    Stream(DataStreamId),
}

impl Default for FileWriteSource {
    fn default() -> Self {
        Self::Bytes(Vec::new())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteFileRequest {
    pub location: FileSystemLocation,
    pub source: FileWriteSource,
    /// Create missing parent directories in the addressed provider.
    pub create_parents: bool,
    /// Replace an existing regular file. The safe default is `false`.
    pub overwrite: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteFileResult {
    pub byte_len: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDirectoryRequest {
    pub location: FileSystemLocation,
    pub recursive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveEntryRequest {
    pub location: FileSystemLocation,
    pub recursive: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDirectoryRequest {
    pub directory: DirectoryHandleId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemError {
    pub code: String,
    pub message: String,
}

impl FileSystemError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "filesystem operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

macro_rules! filesystem_capability {
    ($type:ident, $request:ty, $ok:ty, $constant:ident, $name:literal) => {
        pub struct $type;
        impl OperationCapability for $type {
            type Request = $request;
            type Ok = $ok;
            type Err = FileSystemError;
        }
        pub const $constant: CapabilityType<$type> = CapabilityType::new($name);
    };
}

filesystem_capability!(
    PickDirectoryCapability,
    PickDirectoryRequest,
    PickDirectoryResult,
    PICK_DIRECTORY,
    "fission.fs.pick_directory"
);
filesystem_capability!(
    RestoreDirectoryCapability,
    RestoreDirectoryRequest,
    RestoreDirectoryResult,
    RESTORE_DIRECTORY,
    "fission.fs.restore_directory"
);
filesystem_capability!(
    ForgetDirectoryCapability,
    ForgetDirectoryRequest,
    (),
    FORGET_DIRECTORY,
    "fission.fs.forget_directory"
);
filesystem_capability!(
    DirectoryPermissionCapability,
    DirectoryPermissionRequest,
    DirectoryPermissionResult,
    DIRECTORY_PERMISSION,
    "fission.fs.directory_permission"
);
filesystem_capability!(
    ListDirectoryCapability,
    ListDirectoryRequest,
    ListDirectoryResult,
    LIST_DIRECTORY,
    "fission.fs.list_directory"
);
filesystem_capability!(
    StatEntryCapability,
    StatEntryRequest,
    StatEntryResult,
    STAT_ENTRY,
    "fission.fs.stat_entry"
);
filesystem_capability!(
    ReadFileCapability,
    ReadFileRequest,
    ReadFileResult,
    READ_FILE,
    "fission.fs.read_file"
);
filesystem_capability!(
    WriteFileCapability,
    WriteFileRequest,
    WriteFileResult,
    WRITE_FILE,
    "fission.fs.write_file"
);
filesystem_capability!(
    CreateDirectoryCapability,
    CreateDirectoryRequest,
    (),
    CREATE_DIRECTORY,
    "fission.fs.create_directory"
);
filesystem_capability!(
    RemoveEntryCapability,
    RemoveEntryRequest,
    (),
    REMOVE_ENTRY,
    "fission.fs.remove_entry"
);
filesystem_capability!(
    ReleaseDirectoryCapability,
    ReleaseDirectoryRequest,
    (),
    RELEASE_DIRECTORY,
    "fission.fs.release_directory"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_preserve_provider_input_without_framework_validation() {
        for path in [
            "/etc",
            "../secret",
            "folder/../secret",
            "folder\\secret",
            "C:/secret",
            "C:secret",
            "folder//file",
            "folder/",
        ] {
            assert_eq!(FileSystemPath::new(path).as_str(), path);
        }
        assert_eq!(FileSystemPath::new(""), FileSystemPath::root());
        assert_eq!(
            FileSystemPath::new("folder").join("file.txt").as_str(),
            "folder/file.txt"
        );
    }

    #[test]
    fn paths_round_trip_without_reinterpretation() {
        assert_eq!(
            serde_json::from_str::<FileSystemPath>(r#""../secret""#).unwrap(),
            FileSystemPath::new("../secret")
        );
    }
}
