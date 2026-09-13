//! User-granted directory access.
//!
//! The capability deliberately exposes opaque directory handles and portable
//! relative paths rather than host filesystem paths. A shell remains the
//! authority for the directory selected by the user and for every operation
//! performed beneath it.

use crate::capability::{CapabilityType, OperationCapability};
use crate::DataStreamId;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::fmt;

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

/// A validated, slash-separated path relative to a granted directory.
///
/// The empty path denotes the granted directory itself. Absolute paths,
/// parent traversal, platform separators, drive prefixes, and NUL bytes are
/// rejected so the same request has the same meaning on every target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FileSystemPath(String);

impl FileSystemPath {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn new(path: impl Into<String>) -> Result<Self, FileSystemPathError> {
        let path = path.into();
        validate_relative_path(&path)?;
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|component| !component.is_empty())
    }

    /// Returns a child path after applying the same portable validation.
    pub fn join(&self, child: &str) -> Result<Self, FileSystemPathError> {
        if self.is_root() {
            Self::new(child)
        } else {
            Self::new(format!("{}/{child}", self.0))
        }
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

impl TryFrom<String> for FileSystemPath {
    type Error = FileSystemPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for FileSystemPath {
    type Error = FileSystemPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for FileSystemPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = String::deserialize(deserializer)?;
        Self::new(path).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSystemPathError;

impl fmt::Display for FileSystemPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("filesystem paths must be relative slash-separated paths without empty, `.` or `..` components")
    }
}

impl std::error::Error for FileSystemPathError {}

fn validate_relative_path(path: &str) -> Result<(), FileSystemPathError> {
    let first_component = path.split('/').next().unwrap_or_default();
    let has_drive_prefix = first_component.len() == 2
        && first_component.as_bytes()[0].is_ascii_alphabetic()
        && first_component.as_bytes()[1] == b':';
    if path.contains('\0')
        || path.contains('\\')
        || has_drive_prefix
        || path.starts_with('/')
        || path.ends_with('/')
    {
        return Err(FileSystemPathError);
    }
    if path.is_empty() {
        return Ok(());
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(FileSystemPathError);
    }
    Ok(())
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemEntry {
    pub name: String,
    pub path: FileSystemPath,
    pub kind: FileSystemEntryKind,
    pub byte_len: Option<u64>,
    /// Milliseconds since the Unix epoch when the host exposes it.
    pub modified_millis: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDirectoryRequest {
    pub directory: DirectoryHandleId,
    pub path: FileSystemPath,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDirectoryResult {
    pub entries: Vec<FileSystemEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatEntryRequest {
    pub directory: DirectoryHandleId,
    pub path: FileSystemPath,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatEntryResult {
    pub entry: FileSystemEntry,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub directory: DirectoryHandleId,
    pub path: FileSystemPath,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteFileRequest {
    pub directory: DirectoryHandleId,
    pub path: FileSystemPath,
    pub source: FileWriteSource,
    /// Create missing parent directories beneath the granted root.
    pub create_parents: bool,
    /// Replace an existing regular file. The safe default is `false`.
    pub overwrite: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteFileResult {
    pub byte_len: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDirectoryRequest {
    pub directory: DirectoryHandleId,
    pub path: FileSystemPath,
    pub recursive: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveEntryRequest {
    pub directory: DirectoryHandleId,
    pub path: FileSystemPath,
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
    fn relative_paths_reject_host_specific_and_escaping_forms() {
        for invalid in [
            "/etc",
            "../secret",
            "folder/../secret",
            "folder\\secret",
            "C:/secret",
            "C:secret",
            "folder//file",
            "folder/",
        ] {
            assert!(
                FileSystemPath::new(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert_eq!(FileSystemPath::new("").unwrap(), FileSystemPath::root());
        assert_eq!(
            FileSystemPath::new("folder/file.txt").unwrap().as_str(),
            "folder/file.txt"
        );
        assert_eq!(
            FileSystemPath::new("notes/draft:one.md").unwrap().as_str(),
            "notes/draft:one.md"
        );
        assert_eq!(
            FileSystemPath::new("folder")
                .unwrap()
                .join("file.txt")
                .unwrap()
                .as_str(),
            "folder/file.txt"
        );
    }

    #[test]
    fn invalid_paths_are_rejected_during_deserialization() {
        assert!(serde_json::from_str::<FileSystemPath>(r#""../secret""#).is_err());
        assert_eq!(
            serde_json::from_str::<FileSystemPath>(r#""notes/today.md""#)
                .unwrap()
                .as_str(),
            "notes/today.md"
        );
    }
}
