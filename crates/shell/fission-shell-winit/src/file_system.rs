use base64::Engine;
use fission_core::{
    Bytes, CapabilityCtx, CreateDirectoryRequest, DirectoryHandle, DirectoryHandleId,
    DirectoryPermissionRequest, DirectoryPermissionResult, FileSystemAccessMode, FileSystemEntry,
    FileSystemEntryKind, FileSystemError, FileSystemLocation, FileSystemPermission,
    FileWriteSource, FissionDataStreamError, FissionDataStreamErrorKind, ForgetDirectoryRequest,
    ListDirectoryRequest, ListDirectoryResult, PickDirectoryRequest, PickDirectoryResult,
    ReadFileRequest, ReadFileResult, ReleaseDirectoryRequest, RemoveEntryRequest,
    RestoreDirectoryRequest, RestoreDirectoryResult, StatEntryRequest, StatEntryResult,
    WriteFileRequest, WriteFileResult, CREATE_DIRECTORY, DIRECTORY_PERMISSION, FORGET_DIRECTORY,
    LIST_DIRECTORY, PICK_DIRECTORY, READ_FILE, RELEASE_DIRECTORY, REMOVE_ENTRY, RESTORE_DIRECTORY,
    STAT_ENTRY, WRITE_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use std::collections::HashMap;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::UNIX_EPOCH;

const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;
const PERSISTED_DIRECTORY_VERSION: &[u8] = b"fission-directory-v1\0";

#[derive(Clone)]
pub(crate) struct NativeDirectoryGrant {
    pub(crate) root: PathBuf,
    pub(crate) name: String,
    pub(crate) access: FileSystemAccessMode,
    // Some hosts require a live grant object for the handle's full lifetime.
    pub(crate) _lease: Option<Arc<dyn Send + Sync>>,
}

#[derive(Default)]
pub(crate) struct NativeDirectoryRegistry {
    next_id: AtomicU64,
    grants: Mutex<HashMap<DirectoryHandleId, NativeDirectoryGrant>>,
}

impl NativeDirectoryRegistry {
    pub(crate) fn insert(&self, grant: NativeDirectoryGrant) -> DirectoryHandle {
        let id = DirectoryHandleId(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        let handle = DirectoryHandle {
            id,
            name: grant.name.clone(),
            access: grant.access,
            permission: FileSystemPermission::Granted,
        };
        self.grants.lock().unwrap().insert(id, grant);
        handle
    }

    fn get(&self, id: DirectoryHandleId) -> Result<NativeDirectoryGrant, FileSystemError> {
        self.grants
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                FileSystemError::new("invalid_handle", "the directory handle is no longer active")
            })
    }

    fn release(&self, id: DirectoryHandleId) -> Result<(), FileSystemError> {
        if self.grants.lock().unwrap().remove(&id).is_some() {
            Ok(())
        } else {
            Err(FileSystemError::new(
                "invalid_handle",
                "the directory handle is no longer active",
            ))
        }
    }
}

#[cfg(not(target_os = "ios"))]
pub(crate) fn register_file_system_capabilities(
    async_registry: &mut AsyncRegistry,
    application_name: &str,
) -> NativeDirectoryPersistence {
    let persistence = NativeDirectoryPersistence::for_application(application_name);
    register_file_system_capabilities_with_persistence(async_registry, persistence.clone());
    persistence
}

#[cfg(not(target_os = "ios"))]
fn register_file_system_capabilities_with_persistence(
    async_registry: &mut AsyncRegistry,
    persistence: NativeDirectoryPersistence,
) {
    let registry = Arc::new(NativeDirectoryRegistry::default());

    let grants = registry.clone();
    let selected_directories = persistence.clone();
    async_registry.register_operation_capability(
        PICK_DIRECTORY,
        move |request: PickDirectoryRequest, _| {
            let grants = grants.clone();
            let selected_directories = selected_directories.clone();
            async move {
                if let Some(key) = request.persistence_key.as_deref() {
                    selected_directories.key_path(key)?;
                }
                let Some(root) = rfd::FileDialog::new().pick_folder() else {
                    return Ok(PickDirectoryResult { directory: None });
                };
                validate_selected_root(&root)?;
                if let Some(key) = request.persistence_key.as_deref() {
                    selected_directories.save(key, &root)?;
                }
                let name = directory_name(&root);
                let directory = grants.insert(NativeDirectoryGrant {
                    root,
                    name,
                    access: request.access,
                    _lease: None,
                });
                Ok(PickDirectoryResult {
                    directory: Some(directory),
                })
            }
        },
    );

    let grants = registry.clone();
    let saved_directories = persistence.clone();
    async_registry.register_operation_capability(
        RESTORE_DIRECTORY,
        move |request: RestoreDirectoryRequest, _| {
            let grants = grants.clone();
            let saved_directories = saved_directories.clone();
            async move {
                let Some(root) = saved_directories.restore(&request.persistence_key)? else {
                    return Ok(RestoreDirectoryResult { directory: None });
                };
                validate_selected_root(&root)?;
                let name = directory_name(&root);
                let directory = grants.insert(NativeDirectoryGrant {
                    root,
                    name,
                    access: request.access,
                    _lease: None,
                });
                Ok(RestoreDirectoryResult {
                    directory: Some(directory),
                })
            }
        },
    );
    async_registry.register_operation_capability(
        FORGET_DIRECTORY,
        move |request: ForgetDirectoryRequest, _| {
            let saved_directories = persistence.clone();
            async move { saved_directories.forget(&request.persistence_key) }
        },
    );

    register_native_file_system_operations(async_registry, registry);
}

#[cfg(not(target_os = "ios"))]
fn directory_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("selected-folder")
        .to_string()
}

/// Persists only the selected path. The directory handle remains runtime-owned,
/// and every later filesystem operation is still subject to OS permissions.
/// The display title alone is not an app identity: include the executable path
/// so unrelated same-title apps cannot restore one another's selections. An
/// app moved to a new executable path must ask the user to pick its folder again;
/// the previous path's record is not removed automatically.
/// This path reference does not replace a macOS security-scoped bookmark for
/// sandboxed distribution; those apps must re-pick if the sandbox rejects it.
#[cfg(not(target_os = "ios"))]
#[derive(Clone)]
pub(crate) struct NativeDirectoryPersistence {
    root: Arc<Mutex<Result<PathBuf, FileSystemError>>>,
}

#[cfg(not(target_os = "ios"))]
impl NativeDirectoryPersistence {
    fn for_application(application_name: &str) -> Self {
        Self {
            root: Arc::new(Mutex::new(application_persistence_root(application_name))),
        }
    }

    pub(crate) fn set_application_name(&self, application_name: &str) {
        *self.root.lock().unwrap() = application_persistence_root(application_name);
    }

    #[cfg(test)]
    fn at(root: PathBuf) -> Self {
        Self {
            root: Arc::new(Mutex::new(Ok(root))),
        }
    }

    fn key_path(&self, key: &str) -> Result<PathBuf, FileSystemError> {
        if key.trim().is_empty() {
            return Err(FileSystemError::new(
                "invalid_persistence_key",
                "a non-empty persistence key is required",
            ));
        }
        let root = self.root.lock().unwrap().clone()?;
        let path = encoded_path_components(root, key.as_bytes());
        Ok(path.join("directory"))
    }

    fn save(&self, key: &str, directory: &Path) -> Result<(), FileSystemError> {
        let path = self.key_path(key)?;
        let parent = path.parent().expect("directory record has a parent");
        create_private_directory(parent)?;
        let root = self.root.lock().unwrap().clone()?;
        restrict_private_directory(&root)?;
        let bytes = native_path_bytes(directory);
        let temporary = parent.join(format!(
            "directory.tmp-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let write_result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|error| io_error("persistence_failed", &temporary, error))?;
            file.write_all(PERSISTED_DIRECTORY_VERSION)
                .and_then(|_| file.write_all(&bytes))
                .and_then(|_| file.sync_all())
                .map_err(|error| io_error("persistence_failed", &temporary, error))?;
            drop(file);
            std::fs::rename(&temporary, &path)
                .map_err(|error| io_error("persistence_failed", &path, error))
        })();
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        write_result
    }

    fn restore(&self, key: &str) -> Result<Option<PathBuf>, FileSystemError> {
        let path = self.key_path(key)?;
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error("restore_failed", &path, error)),
        };
        let encoded = bytes
            .strip_prefix(PERSISTED_DIRECTORY_VERSION)
            .ok_or_else(|| {
                FileSystemError::new("restore_failed", "the saved directory record is invalid")
            })?;
        path_from_native_bytes(encoded).map(Some)
    }

    fn forget(&self, key: &str) -> Result<(), FileSystemError> {
        let path = self.key_path(key)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error("forget_failed", &path, error)),
        }
    }
}

#[cfg(not(target_os = "ios"))]
fn application_persistence_root(application_name: &str) -> Result<PathBuf, FileSystemError> {
    application_data_directory()
        .ok_or_else(|| {
            FileSystemError::new(
                "persistence_unavailable",
                "the operating system did not expose an application-data directory",
            )
        })
        .and_then(|root| {
            let executable = std::env::current_exe().map_err(|error| {
                FileSystemError::new(
                    "persistence_unavailable",
                    format!("the current executable path is unavailable: {error}"),
                )
            })?;
            Ok(persistence_root(root, application_name, &executable))
        })
}

#[cfg(not(target_os = "ios"))]
fn persistence_root(data_root: PathBuf, application_name: &str, executable: &Path) -> PathBuf {
    let title_root = data_root.join("fission").join("directory-grants");
    let app_root = if application_name.is_empty() {
        title_root.join("~")
    } else {
        encoded_path_components(title_root, application_name.as_bytes())
    };
    encoded_path_components(app_root, &native_path_bytes(executable))
}

#[cfg(not(target_os = "ios"))]
fn encoded_path_components(mut root: PathBuf, bytes: &[u8]) -> PathBuf {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    // Split long identities and keys below ordinary host component limits.
    for chunk in encoded.as_bytes().chunks(120) {
        root.push(std::str::from_utf8(chunk).expect("base64 is ASCII"));
    }
    root
}

#[cfg(all(not(target_os = "ios"), unix))]
fn create_private_directory(path: &Path) -> Result<(), FileSystemError> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder
        .create(path)
        .map_err(|error| io_error("persistence_failed", path, error))
}

#[cfg(all(not(target_os = "ios"), not(unix)))]
fn create_private_directory(path: &Path) -> Result<(), FileSystemError> {
    std::fs::create_dir_all(path).map_err(|error| io_error("persistence_failed", path, error))
}

#[cfg(all(not(target_os = "ios"), unix))]
fn restrict_private_directory(path: &Path) -> Result<(), FileSystemError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| io_error("persistence_failed", path, error))
}

#[cfg(all(not(target_os = "ios"), not(unix)))]
fn restrict_private_directory(_path: &Path) -> Result<(), FileSystemError> {
    Ok(())
}

#[cfg(not(target_os = "ios"))]
static NEXT_DIRECTORY_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(all(not(target_os = "ios"), unix))]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(all(not(target_os = "ios"), unix))]
fn path_from_native_bytes(bytes: &[u8]) -> Result<PathBuf, FileSystemError> {
    use std::os::unix::ffi::OsStringExt;
    if bytes.is_empty() {
        return Err(FileSystemError::new(
            "restore_failed",
            "the saved directory path is empty",
        ));
    }
    Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
}

#[cfg(windows)]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_native_bytes(bytes: &[u8]) -> Result<PathBuf, FileSystemError> {
    use std::os::windows::ffi::OsStringExt;
    if bytes.is_empty() || bytes.len() % 2 != 0 {
        return Err(FileSystemError::new(
            "restore_failed",
            "the saved directory path is invalid",
        ));
    }
    let wide = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Ok(std::ffi::OsString::from_wide(&wide).into())
}

#[cfg(all(not(target_os = "ios"), windows))]
fn application_data_directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(Into::into)
}

#[cfg(target_os = "macos")]
fn application_data_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support"))
}

#[cfg(not(any(target_os = "ios", target_os = "macos", windows)))]
fn application_data_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local/share"))
        })
}

pub(crate) fn register_native_file_system_operations(
    async_registry: &mut AsyncRegistry,
    registry: Arc<NativeDirectoryRegistry>,
) {
    let grants = registry.clone();
    async_registry.register_operation_capability(
        DIRECTORY_PERMISSION,
        move |request: DirectoryPermissionRequest, _| {
            let grants = grants.clone();
            async move {
                grants.get(request.directory)?;
                Ok(DirectoryPermissionResult {
                    permission: FileSystemPermission::Granted,
                })
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        LIST_DIRECTORY,
        move |request: ListDirectoryRequest, _| {
            let grants = grants.clone();
            async move {
                let resolved = resolve_location(&grants, &request.location)?;
                let path = resolved.path;
                let mut entries = std::fs::read_dir(&path)
                    .map_err(|error| io_error("read_directory_failed", &path, error))?
                    .map(|entry| {
                        let entry = entry
                            .map_err(|error| io_error("read_directory_failed", &path, error))?;
                        let name = entry.file_name().into_string().map_err(|_| {
                            FileSystemError::new(
                                "unsupported_entry",
                                "the directory contains a name that is not valid UTF-8",
                            )
                        })?;
                        let child_location = request.location.join(&name);
                        Ok(entry_from_metadata(
                            name,
                            child_location,
                            std::fs::symlink_metadata(entry.path()).map_err(|error| {
                                io_error("metadata_failed", &entry.path(), error)
                            })?,
                        ))
                    })
                    .collect::<Result<Vec<_>, FileSystemError>>()?;
                entries.sort_by(|left, right| left.name.cmp(&right.name));
                Ok(ListDirectoryResult { entries })
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        STAT_ENTRY,
        move |request: StatEntryRequest, _| {
            let grants = grants.clone();
            async move {
                let resolved = resolve_location(&grants, &request.location)?;
                let path = resolved.path;
                let metadata = std::fs::metadata(&path)
                    .map_err(|error| io_error("metadata_failed", &path, error))?;
                let name = entry_name(&path, resolved.grant.as_ref());
                Ok(StatEntryResult {
                    entry: entry_from_metadata(name, request.location, metadata),
                })
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        READ_FILE,
        move |request: ReadFileRequest, ctx: CapabilityCtx| {
            let grants = grants.clone();
            async move {
                let path = resolve_location(&grants, &request.location)?.path;
                let metadata = std::fs::metadata(&path)
                    .map_err(|error| io_error("metadata_failed", &path, error))?;
                if !metadata.is_file() {
                    return Err(FileSystemError::new(
                        "not_a_file",
                        "the requested entry is not a regular file",
                    ));
                }
                Ok(ReadFileResult {
                    stream: ctx.register_data_stream(Box::pin(FileDataStream::new(path))),
                    byte_len: Some(metadata.len()),
                    modified_millis: modified_millis(&metadata),
                })
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        WRITE_FILE,
        move |request: WriteFileRequest, ctx: CapabilityCtx| {
            let grants = grants.clone();
            async move {
                let resolved = resolve_location(&grants, &request.location)?;
                let path = resolved.path;
                // Acquire a stream before creating directories or opening the
                // destination. An invalid or already-consumed stream must not
                // truncate an existing user file or leave an empty new file.
                let mut source_stream = match &request.source {
                    FileWriteSource::Bytes(_) => None,
                    FileWriteSource::Stream(id) => {
                        Some(ctx.open_data_stream(*id).map_err(|error| {
                            FileSystemError::new("stream_open_failed", error.to_string())
                        })?)
                    }
                };
                if request.create_parents {
                    if let Some(parent) = path
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                    {
                        std::fs::create_dir_all(parent)
                            .map_err(|error| io_error("create_directory_failed", parent, error))?;
                    }
                }
                let mut options = std::fs::OpenOptions::new();
                options.write(true);
                if request.overwrite {
                    options.create(true).truncate(true);
                } else {
                    options.create_new(true);
                }
                let mut file = options
                    .open(&path)
                    .map_err(|error| io_error("write_failed", &path, error))?;
                let byte_len = match request.source {
                    FileWriteSource::Bytes(bytes) => {
                        file.write_all(&bytes)
                            .map_err(|error| io_error("write_failed", &path, error))?;
                        bytes.len() as u64
                    }
                    FileWriteSource::Stream(_) => {
                        let stream = source_stream.as_mut().expect("stream was opened above");
                        let mut written = 0_u64;
                        while let Some(chunk) =
                            std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
                        {
                            let chunk = chunk.map_err(|error| {
                                FileSystemError::new("stream_read_failed", error.to_string())
                            })?;
                            file.write_all(&chunk)
                                .map_err(|error| io_error("write_failed", &path, error))?;
                            written = written.saturating_add(chunk.len() as u64);
                        }
                        written
                    }
                };
                file.flush()
                    .map_err(|error| io_error("write_failed", &path, error))?;
                Ok(WriteFileResult { byte_len })
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        CREATE_DIRECTORY,
        move |request: CreateDirectoryRequest, _| {
            let grants = grants.clone();
            async move {
                let resolved = resolve_location(&grants, &request.location)?;
                let path = resolved.path;
                let result = if request.recursive {
                    std::fs::create_dir_all(&path)
                } else {
                    std::fs::create_dir(&path)
                };
                result.map_err(|error| io_error("create_directory_failed", &path, error))
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        REMOVE_ENTRY,
        move |request: RemoveEntryRequest, _| {
            let grants = grants.clone();
            async move {
                let resolved = resolve_location(&grants, &request.location)?;
                let path = resolved.path;
                let metadata = std::fs::symlink_metadata(&path)
                    .map_err(|error| io_error("metadata_failed", &path, error))?;
                let result = if metadata.is_dir() {
                    if request.recursive {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_dir(&path)
                    }
                } else {
                    std::fs::remove_file(&path)
                };
                result.map_err(|error| io_error("remove_failed", &path, error))
            }
        },
    );

    async_registry.register_operation_capability(
        RELEASE_DIRECTORY,
        move |request: ReleaseDirectoryRequest, _| {
            let registry = registry.clone();
            async move { registry.release(request.directory) }
        },
    );
}

fn validate_selected_root(root: &Path) -> Result<(), FileSystemError> {
    let metadata =
        std::fs::metadata(root).map_err(|error| io_error("metadata_failed", root, error))?;
    if !metadata.is_dir() {
        return Err(FileSystemError::new(
            "not_a_directory",
            "the selected entry is not a directory",
        ));
    }
    Ok(())
}

struct ResolvedNativeLocation {
    path: PathBuf,
    grant: Option<NativeDirectoryGrant>,
}

fn resolve_location(
    registry: &NativeDirectoryRegistry,
    location: &FileSystemLocation,
) -> Result<ResolvedNativeLocation, FileSystemError> {
    match location {
        FileSystemLocation::Path(path) => Ok(ResolvedNativeLocation {
            path: PathBuf::from(path.as_str()),
            grant: None,
        }),
        FileSystemLocation::Directory { directory, path } => {
            let grant = registry.get(*directory)?;
            Ok(ResolvedNativeLocation {
                path: grant.root.join(path.as_str()),
                grant: Some(grant),
            })
        }
    }
}

fn entry_name(path: &Path, grant: Option<&NativeDirectoryGrant>) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .or_else(|| grant.map(|grant| grant.name.clone()))
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn entry_from_metadata(
    name: String,
    location: FileSystemLocation,
    metadata: Metadata,
) -> FileSystemEntry {
    let kind = if metadata.is_file() {
        FileSystemEntryKind::File
    } else if metadata.is_dir() {
        FileSystemEntryKind::Directory
    } else {
        FileSystemEntryKind::Other
    };
    FileSystemEntry {
        name,
        location,
        kind,
        byte_len: metadata.is_file().then(|| metadata.len()),
        modified_millis: modified_millis(&metadata),
    }
}

fn modified_millis(metadata: &Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
}

fn io_error(code: &str, _path: &Path, error: std::io::Error) -> FileSystemError {
    let code = match error.kind() {
        std::io::ErrorKind::NotFound => "not_found",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::AlreadyExists => "already_exists",
        std::io::ErrorKind::InvalidInput => "invalid_path",
        _ => code,
    };
    FileSystemError::new(code, format!("filesystem operation failed: {error}"))
}

struct FileDataStream {
    path: PathBuf,
    file: Option<File>,
    done: bool,
}

impl FileDataStream {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: None,
            done: false,
        }
    }
}

impl Stream for FileDataStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }
        if self.file.is_none() {
            match File::open(&self.path) {
                Ok(file) => self.file = Some(file),
                Err(error) => {
                    self.done = true;
                    return Poll::Ready(Some(Err(FissionDataStreamError::new(
                        FissionDataStreamErrorKind::Io,
                        error.to_string(),
                    ))));
                }
            }
        }
        let mut buffer = vec![0; FILE_STREAM_CHUNK_SIZE];
        match self.file.as_mut().unwrap().read(&mut buffer) {
            Ok(0) => {
                self.done = true;
                Poll::Ready(None)
            }
            Ok(read) => {
                buffer.truncate(read);
                Poll::Ready(Some(Ok(Bytes::from(buffer))))
            }
            Err(error) => {
                self.done = true;
                Poll::Ready(Some(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::Io,
                    error.to_string(),
                ))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "ios"))]
    use fission_shell::async_host::AsyncMessage;

    #[cfg(not(target_os = "ios"))]
    fn persistence_test_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fission-directory-persistence-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn native_directory_reference_survives_a_new_registry_and_can_be_forgotten() {
        let test_root = persistence_test_root();
        let picked = test_root.join("picked");
        std::fs::create_dir_all(&picked).unwrap();
        let storage = NativeDirectoryPersistence::at(test_root.join("grants"));

        storage.save("talker/output", &picked).unwrap();
        let restored = storage.restore("talker/output").unwrap().unwrap();
        validate_selected_root(&restored).unwrap();
        let registry = NativeDirectoryRegistry::default();
        let handle = registry.insert(NativeDirectoryGrant {
            root: restored,
            name: directory_name(&picked),
            access: FileSystemAccessMode::ReadWrite,
            _lease: None,
        });
        assert_eq!(handle.name, "picked");
        assert_eq!(
            resolve_location(&registry, &FileSystemLocation::directory_root(handle.id))
                .unwrap()
                .path,
            picked
        );

        storage.forget("talker/output").unwrap();
        storage.forget("talker/output").unwrap();
        assert!(storage.restore("talker/output").unwrap().is_none());
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn registered_restore_and_forget_capabilities_use_the_saved_reference() {
        let test_root = persistence_test_root();
        let picked = test_root.join("picked");
        std::fs::create_dir_all(&picked).unwrap();
        let storage = NativeDirectoryPersistence::at(test_root.join("grants"));
        storage.save("output", &picked).unwrap();
        let mut registry = AsyncRegistry::new();
        register_file_system_capabilities_with_persistence(&mut registry, storage);
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = Arc::new(|| {});

        assert!(registry.spawn_capability(
            RESTORE_DIRECTORY.name,
            1,
            serde_json::to_vec(&RestoreDirectoryRequest {
                persistence_key: "output".into(),
                access: FileSystemAccessMode::ReadWrite,
            })
            .unwrap(),
            None,
            None,
            None,
            &tx,
            wake.clone(),
        ));
        let restored = match rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap() {
            AsyncMessage::CapabilityOk { payload, .. } => {
                serde_json::from_slice::<RestoreDirectoryResult>(&payload).unwrap()
            }
            _ => panic!("restore directory capability did not succeed"),
        };
        let directory = restored.directory.unwrap();
        assert_eq!(directory.name, "picked");
        assert_eq!(directory.access, FileSystemAccessMode::ReadWrite);

        assert!(registry.spawn_capability(
            STAT_ENTRY.name,
            2,
            serde_json::to_vec(&StatEntryRequest {
                location: FileSystemLocation::directory_root(directory.id),
            })
            .unwrap(),
            None,
            None,
            None,
            &tx,
            wake.clone(),
        ));
        let entry = match rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap() {
            AsyncMessage::CapabilityOk { payload, .. } => {
                serde_json::from_slice::<StatEntryResult>(&payload)
                    .unwrap()
                    .entry
            }
            _ => panic!("restored directory was not usable by filesystem capabilities"),
        };
        assert_eq!(entry.kind, FileSystemEntryKind::Directory);

        assert!(registry.spawn_capability(
            FORGET_DIRECTORY.name,
            3,
            serde_json::to_vec(&ForgetDirectoryRequest {
                persistence_key: "output".into(),
            })
            .unwrap(),
            None,
            None,
            None,
            &tx,
            wake,
        ));
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap(),
            AsyncMessage::CapabilityOk { .. }
        ));
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn finalizing_app_title_does_not_replace_custom_filesystem_capabilities() {
        let mut registry = AsyncRegistry::new();
        let persistence = register_file_system_capabilities(&mut registry, "Fission");
        registry.register_operation_capability(
            RESTORE_DIRECTORY,
            |_request: RestoreDirectoryRequest, _| async {
                Ok::<_, FileSystemError>(RestoreDirectoryResult {
                    directory: Some(DirectoryHandle {
                        id: DirectoryHandleId(999),
                        name: "custom provider".into(),
                        access: FileSystemAccessMode::Read,
                        permission: FileSystemPermission::Granted,
                    }),
                })
            },
        );
        persistence.set_application_name("Talker");

        let (tx, rx) = std::sync::mpsc::channel();
        assert!(registry.spawn_capability(
            RESTORE_DIRECTORY.name,
            1,
            serde_json::to_vec(&RestoreDirectoryRequest {
                persistence_key: "output".into(),
                access: FileSystemAccessMode::Read,
            })
            .unwrap(),
            None,
            None,
            None,
            &tx,
            Arc::new(|| {}),
        ));
        let restored = match rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap() {
            AsyncMessage::CapabilityOk { payload, .. } => {
                serde_json::from_slice::<RestoreDirectoryResult>(&payload).unwrap()
            }
            _ => panic!("custom restore directory capability did not succeed"),
        };
        assert_eq!(restored.directory.unwrap().name, "custom provider");
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn persistence_keys_are_isolated_and_replacing_one_does_not_change_another() {
        let test_root = persistence_test_root();
        let first = test_root.join("first");
        let second = test_root.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let storage = NativeDirectoryPersistence::at(test_root.join("grants"));

        storage.save("output", &first).unwrap();
        storage.save("output-2", &second).unwrap();
        storage.save("output", &second).unwrap();
        assert_eq!(storage.restore("output").unwrap(), Some(second.clone()));
        assert_eq!(storage.restore("output-2").unwrap(), Some(second));
        assert_eq!(storage.restore("other").unwrap(), None);
        assert_eq!(
            storage.restore("  ").unwrap_err().code,
            "invalid_persistence_key"
        );
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn same_title_applications_with_different_executables_have_separate_grants() {
        let test_root = persistence_test_root();
        let first_root =
            persistence_root(test_root.clone(), "Talker", Path::new("/app-one/talker"));
        let second_root =
            persistence_root(test_root.clone(), "Talker", Path::new("/app-two/talker"));
        assert_ne!(first_root, second_root);
        assert_ne!(
            persistence_root(test_root.clone(), "A/B", Path::new("/app-one/talker")),
            persistence_root(test_root.clone(), "A_B", Path::new("/app-one/talker"))
        );
        let first = NativeDirectoryPersistence::at(first_root);
        let second = NativeDirectoryPersistence::at(second_root);
        let picked = test_root.join("picked");
        std::fs::create_dir_all(&picked).unwrap();
        first.save("output", &picked).unwrap();
        assert_eq!(first.restore("output").unwrap(), Some(picked));
        assert_eq!(second.restore("output").unwrap(), None);
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn final_app_title_updates_shared_persistence_scope() {
        let persistence = NativeDirectoryPersistence::for_application("Before");
        let captured_by_handlers = persistence.clone();
        let before = persistence.root.lock().unwrap().clone().unwrap();
        persistence.set_application_name("After");
        let after = captured_by_handlers.root.lock().unwrap().clone().unwrap();
        assert_ne!(before, after);
        let title_component = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("After");
        assert!(after
            .components()
            .any(|part| part.as_os_str() == std::ffi::OsStr::new(&title_component)));
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn missing_selected_directory_is_reported_as_missing_not_as_an_absent_grant() {
        let test_root = persistence_test_root();
        let picked = test_root.join("picked");
        std::fs::create_dir_all(&picked).unwrap();
        let storage = NativeDirectoryPersistence::at(test_root.join("grants"));
        storage.save("output", &picked).unwrap();
        std::fs::remove_dir(&picked).unwrap();

        let restored = storage.restore("output").unwrap().unwrap();
        assert_eq!(
            validate_selected_root(&restored).unwrap_err().code,
            "not_found"
        );
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(all(not(target_os = "ios"), unix))]
    #[test]
    fn saved_grants_are_private_to_the_current_os_user() {
        use std::os::unix::fs::PermissionsExt;

        let test_root = persistence_test_root();
        let picked = test_root.join("picked");
        std::fs::create_dir_all(&picked).unwrap();
        let grant_root = test_root.join("grants");
        let storage = NativeDirectoryPersistence::at(grant_root.clone());
        storage.save("output", &picked).unwrap();

        assert_eq!(
            std::fs::metadata(&grant_root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let record = storage.key_path("output").unwrap();
        assert_eq!(
            std::fs::metadata(record).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(all(not(target_os = "ios"), unix))]
    #[test]
    fn persisted_directory_reference_preserves_non_utf8_native_paths() {
        use std::os::unix::ffi::OsStringExt;

        let test_root = persistence_test_root();
        let picked = test_root.join(std::ffi::OsString::from_vec(b"picked-\xff".to_vec()));
        std::fs::create_dir_all(&picked).unwrap();
        let storage = NativeDirectoryPersistence::at(test_root.join("grants"));
        storage.save("binary-path", &picked).unwrap();
        assert_eq!(storage.restore("binary-path").unwrap(), Some(picked));
        std::fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn registers_every_directory_operation() {
        let mut registry = AsyncRegistry::new();
        register_file_system_capabilities(&mut registry, "Fission test");

        assert!(registry.has_operation_capability(PICK_DIRECTORY));
        assert!(registry.has_operation_capability(RESTORE_DIRECTORY));
        assert!(registry.has_operation_capability(FORGET_DIRECTORY));
        assert!(registry.has_operation_capability(DIRECTORY_PERMISSION));
        assert!(registry.has_operation_capability(LIST_DIRECTORY));
        assert!(registry.has_operation_capability(STAT_ENTRY));
        assert!(registry.has_operation_capability(READ_FILE));
        assert!(registry.has_operation_capability(WRITE_FILE));
        assert!(registry.has_operation_capability(CREATE_DIRECTORY));
        assert!(registry.has_operation_capability(REMOVE_ENTRY));
        assert!(registry.has_operation_capability(RELEASE_DIRECTORY));
    }

    #[test]
    fn native_directory_permissions_are_owned_by_the_os() {
        let grant = NativeDirectoryGrant {
            root: PathBuf::new(),
            name: "project".into(),
            access: FileSystemAccessMode::Read,
            _lease: None,
        };
        let registry = NativeDirectoryRegistry::default();
        let handle = registry.insert(grant);
        let resolved =
            resolve_location(&registry, &FileSystemLocation::directory_root(handle.id)).unwrap();
        assert_eq!(resolved.grant.unwrap().access, FileSystemAccessMode::Read);
    }

    #[test]
    fn direct_native_paths_are_passed_through_unchanged() {
        let registry = NativeDirectoryRegistry::default();
        let path = if cfg!(windows) {
            r"C:\etc\my-app\config.toml"
        } else {
            "/etc/my-app/config.toml"
        };
        let resolved = resolve_location(&registry, &FileSystemLocation::path(path)).unwrap();
        assert_eq!(resolved.path, PathBuf::from(path));
        assert!(resolved.grant.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn native_directory_locations_use_os_join_semantics() {
        let registry = NativeDirectoryRegistry::default();
        let directory = registry.insert(NativeDirectoryGrant {
            root: PathBuf::from("/tmp/project"),
            name: "project".into(),
            access: FileSystemAccessMode::ReadWrite,
            _lease: None,
        });

        let parent = resolve_location(
            &registry,
            &FileSystemLocation::directory(directory.id, "../outside"),
        )
        .unwrap();
        assert_eq!(parent.path, PathBuf::from("/tmp/project/../outside"));

        let absolute = resolve_location(
            &registry,
            &FileSystemLocation::directory(directory.id, "/etc/my-app/config.toml"),
        )
        .unwrap();
        assert_eq!(absolute.path, PathBuf::from("/etc/my-app/config.toml"));
    }

    #[cfg(unix)]
    #[test]
    fn native_paths_leave_symbolic_link_policy_to_the_os() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "fission-filesystem-symlink-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        symlink(&target, root.join("linked")).unwrap();
        let grant = NativeDirectoryGrant {
            root: root.clone(),
            name: "project".into(),
            access: FileSystemAccessMode::ReadWrite,
            _lease: None,
        };

        let registry = NativeDirectoryRegistry::default();
        let directory = registry.insert(grant);
        let resolved = resolve_location(
            &registry,
            &FileSystemLocation::directory(directory.id, "linked"),
        )
        .unwrap();
        assert!(std::fs::metadata(resolved.path).unwrap().is_dir());

        std::fs::remove_dir_all(root).unwrap();
    }
}
