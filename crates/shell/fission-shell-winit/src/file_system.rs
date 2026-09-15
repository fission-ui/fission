use fission_core::{
    Bytes, CapabilityCtx, CreateDirectoryRequest, DirectoryHandle, DirectoryHandleId,
    DirectoryPermissionRequest, DirectoryPermissionResult, FileSystemAccessMode, FileSystemEntry,
    FileSystemEntryKind, FileSystemError, FileSystemLocation, FileSystemPath, FileSystemPermission,
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
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::UNIX_EPOCH;

const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;

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
pub fn register_file_system_capabilities(async_registry: &mut AsyncRegistry) {
    let registry = Arc::new(NativeDirectoryRegistry::default());

    let grants = registry.clone();
    async_registry.register_operation_capability(
        PICK_DIRECTORY,
        move |request: PickDirectoryRequest, _| {
            let grants = grants.clone();
            async move {
                let Some(root) = rfd::FileDialog::new().pick_folder() else {
                    return Ok(PickDirectoryResult { directory: None });
                };
                validate_selected_root(&root)?;
                let name = root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("selected-folder")
                    .to_string();
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

    async_registry.register_operation_capability(
        RESTORE_DIRECTORY,
        |_request: RestoreDirectoryRequest, _| async {
            Err::<RestoreDirectoryResult, _>(FileSystemError::unsupported("restore_directory"))
        },
    );
    async_registry.register_operation_capability(
        FORGET_DIRECTORY,
        |_request: ForgetDirectoryRequest, _| async {
            Err::<(), _>(FileSystemError::unsupported("forget_directory"))
        },
    );

    register_native_file_system_operations(async_registry, registry);
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
                    if let Some(parent) = path.parent() {
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

    #[test]
    fn registers_every_directory_operation() {
        let mut registry = AsyncRegistry::new();
        register_file_system_capabilities(&mut registry);

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
