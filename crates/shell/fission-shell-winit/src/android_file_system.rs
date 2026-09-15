use crate::android_capabilities::{app_class, AndroidHostContext};
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
use jni::objects::{JClass, JObject, JObjectArray, JString, JValue};
use jni::sys::{jint, jlong};
use jni::JNIEnv;
use std::collections::HashMap;
use std::fs::{File, Metadata};
use std::future::Future;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::time::UNIX_EPOCH;

const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;
const JAVA_HELPER: &str = "rs.fission.runtime.FissionFileSystem";

#[derive(Clone)]
struct AndroidDirectoryGrant {
    uri: String,
    name: String,
    access: FileSystemAccessMode,
}

#[derive(Default)]
struct AndroidDirectoryRegistry {
    next_id: AtomicU64,
    grants: Mutex<HashMap<DirectoryHandleId, AndroidDirectoryGrant>>,
}

impl AndroidDirectoryRegistry {
    fn insert(
        &self,
        grant: AndroidDirectoryGrant,
        permission: FileSystemPermission,
    ) -> DirectoryHandle {
        let id = DirectoryHandleId(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        let handle = DirectoryHandle {
            id,
            name: grant.name.clone(),
            access: grant.access,
            permission,
        };
        self.grants.lock().unwrap().insert(id, grant);
        handle
    }

    fn get(&self, id: DirectoryHandleId) -> Result<AndroidDirectoryGrant, FileSystemError> {
        self.grants
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| fs_error("invalid_handle", "the directory handle is no longer active"))
    }

    fn release(&self, id: DirectoryHandleId) -> Result<(), FileSystemError> {
        self.grants
            .lock()
            .unwrap()
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| fs_error("invalid_handle", "the directory handle is no longer active"))
    }
}

struct PickState {
    result: Mutex<Option<Result<Option<PickedDirectory>, FileSystemError>>>,
    waker: Mutex<Option<Waker>>,
    access: FileSystemAccessMode,
}

struct PickedDirectory {
    uri: String,
    name: String,
}

struct PickFuture(Arc<PickState>);

impl Future for PickFuture {
    type Output = Result<Option<PickedDirectory>, FileSystemError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.0.result.lock().unwrap().take() {
            Poll::Ready(result)
        } else {
            *self.0.waker.lock().unwrap() = Some(cx.waker().clone());
            // The callback can complete between the first result check and
            // storing the waker. Recheck so that race cannot lose its wakeup.
            self.0
                .result
                .lock()
                .unwrap()
                .take()
                .map_or(Poll::Pending, Poll::Ready)
        }
    }
}

fn pending_picks() -> &'static Mutex<HashMap<u64, Arc<PickState>>> {
    static PICKS: OnceLock<Mutex<HashMap<u64, Arc<PickState>>>> = OnceLock::new();
    PICKS.get_or_init(Default::default)
}

fn next_pick_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[no_mangle]
pub extern "system" fn Java_rs_fission_runtime_FissionFileSystem_nativeDirectoryResult(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    request_id: jlong,
    uri: JString<'_>,
    name: JString<'_>,
    flags: jint,
    error: JString<'_>,
) {
    let Some(state) = pending_picks().lock().unwrap().remove(&(request_id as u64)) else {
        return;
    };
    let result = if !error.is_null() {
        match env.get_string(&error) {
            Ok(value) => Err(parse_java_error(value.to_string_lossy().as_ref())),
            Err(value) => Err(fs_error("picker_failed", value.to_string())),
        }
    } else if uri.is_null() {
        Ok(None)
    } else if flags & 1 == 0
        || (matches!(state.access, FileSystemAccessMode::ReadWrite) && flags & 2 == 0)
    {
        Err(fs_error(
            "permission_denied",
            "Android did not grant the requested directory access",
        ))
    } else {
        let uri = env
            .get_string(&uri)
            .map(|value| value.to_string_lossy().into_owned());
        let name = if name.is_null() {
            Ok("selected-folder".to_string())
        } else {
            env.get_string(&name)
                .map(|value| value.to_string_lossy().into_owned())
        };
        match (uri, name) {
            (Ok(uri), Ok(name)) => Ok(Some(PickedDirectory { uri, name })),
            (Err(error), _) | (_, Err(error)) => Err(fs_error("picker_failed", error.to_string())),
        }
    };
    *state.result.lock().unwrap() = Some(result);
    if let Some(waker) = state.waker.lock().unwrap().take() {
        waker.wake();
    }
}

pub(crate) fn register_android_file_system_capabilities(
    async_registry: &mut AsyncRegistry,
    context: AndroidHostContext,
) {
    let registry = Arc::new(AndroidDirectoryRegistry::default());

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        PICK_DIRECTORY,
        move |request: PickDirectoryRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let id = next_pick_id();
                let state = Arc::new(PickState {
                    result: Mutex::new(None),
                    waker: Mutex::new(None),
                    access: request.access,
                });
                pending_picks().lock().unwrap().insert(id, state.clone());
                if let Err(error) = begin_pick(&host, id, &request) {
                    pending_picks().lock().unwrap().remove(&id);
                    return Err(error);
                }
                let directory = PickFuture(state).await?.map(|picked| {
                    grants.insert(
                        AndroidDirectoryGrant {
                            uri: picked.uri,
                            name: picked.name,
                            access: request.access,
                        },
                        FileSystemPermission::Granted,
                    )
                });
                Ok(PickDirectoryResult { directory })
            }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        RESTORE_DIRECTORY,
        move |request: RestoreDirectoryRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let Some(uri) = call_optional_string(
                    &host,
                    "restore",
                    "(Landroid/app/Activity;Ljava/lang/String;)Ljava/lang/String;",
                    &request.persistence_key,
                )?
                else {
                    return Ok(RestoreDirectoryResult { directory: None });
                };
                let name = call_string(
                    &host,
                    "displayName",
                    "(Landroid/app/Activity;Ljava/lang/String;)Ljava/lang/String;",
                    &uri,
                )?;
                let permission = if call_permission(&host, &uri, request.access)? {
                    FileSystemPermission::Granted
                } else {
                    FileSystemPermission::Denied
                };
                let directory = grants.insert(
                    AndroidDirectoryGrant {
                        uri,
                        name,
                        access: request.access,
                    },
                    permission,
                );
                Ok(RestoreDirectoryResult {
                    directory: Some(directory),
                })
            }
        },
    );

    let host = context.clone();
    async_registry.register_operation_capability(
        FORGET_DIRECTORY,
        move |request: ForgetDirectoryRequest, _| {
            let host = host.clone();
            async move { call_void_string(&host, "forget", &request.persistence_key) }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        DIRECTORY_PERMISSION,
        move |request: DirectoryPermissionRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let grant = grants.get(request.directory)?;
                let permission = if access_allows(grant.access, request.access)
                    && call_permission(&host, &grant.uri, request.access)?
                {
                    FileSystemPermission::Granted
                } else {
                    FileSystemPermission::Denied
                };
                Ok(DirectoryPermissionResult { permission })
            }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        LIST_DIRECTORY,
        move |request: ListDirectoryRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let mut entries = match &request.location {
                    FileSystemLocation::Path(path) => list_native(path, &request.location)?,
                    FileSystemLocation::Directory { directory, path } => {
                        let grant = grants.get(*directory)?;
                        call_rows(&host, "list", &grant.uri, path.as_str())?
                            .into_iter()
                            .map(|row| row.into_entry(&request.location))
                            .collect::<Result<Vec<_>, _>>()?
                    }
                };
                entries.sort_by(|left, right| left.name.cmp(&right.name));
                Ok(ListDirectoryResult { entries })
            }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        STAT_ENTRY,
        move |request: StatEntryRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let entry = match &request.location {
                    FileSystemLocation::Path(path) => stat_native(path, request.location.clone())?,
                    FileSystemLocation::Directory { directory, path } => {
                        let grant = grants.get(*directory)?;
                        call_row(&host, "stat", &grant.uri, path.as_str())?
                            .into_entry_at(request.location.clone())?
                    }
                };
                Ok(StatEntryResult { entry })
            }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        READ_FILE,
        move |request: ReadFileRequest, ctx: CapabilityCtx| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let (file, byte_len, modified_millis) = match &request.location {
                    FileSystemLocation::Path(path) => {
                        let native = PathBuf::from(path.as_str());
                        let metadata = std::fs::metadata(&native).map_err(native_io_error)?;
                        if !metadata.is_file() {
                            return Err(fs_error(
                                "not_a_file",
                                "the requested entry is not a file",
                            ));
                        }
                        (
                            File::open(&native).map_err(native_io_error)?,
                            Some(metadata.len()),
                            modified_millis(&metadata),
                        )
                    }
                    FileSystemLocation::Directory { directory, path } => {
                        let grant = grants.get(*directory)?;
                        let row = call_row(&host, "stat", &grant.uri, path.as_str())?;
                        if row.kind != FileSystemEntryKind::File {
                            return Err(fs_error(
                                "not_a_file",
                                "the requested entry is not a file",
                            ));
                        }
                        let fd = call_open(&host, "openRead", &grant.uri, path.as_str(), None)?;
                        (
                            unsafe { File::from_raw_fd(fd) },
                            row.byte_len,
                            row.modified_millis,
                        )
                    }
                };
                Ok(ReadFileResult {
                    stream: ctx.register_data_stream(Box::pin(AndroidFileDataStream {
                        file,
                        finished: false,
                    })),
                    byte_len,
                    modified_millis,
                })
            }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        WRITE_FILE,
        move |request: WriteFileRequest, ctx: CapabilityCtx| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                let mut source = match &request.source {
                    FileWriteSource::Bytes(_) => None,
                    FileWriteSource::Stream(id) => Some(
                        ctx.open_data_stream(*id)
                            .map_err(|error| fs_error("stream_open_failed", error.to_string()))?,
                    ),
                };
                let mut file = match &request.location {
                    FileSystemLocation::Path(path) => {
                        let path = PathBuf::from(path.as_str());
                        if request.create_parents {
                            if let Some(parent) = path.parent() {
                                std::fs::create_dir_all(parent).map_err(native_io_error)?;
                            }
                        }
                        let mut options = std::fs::OpenOptions::new();
                        options.write(true);
                        if request.overwrite {
                            options.create(true).truncate(true);
                        } else {
                            options.create_new(true);
                        }
                        options.open(path).map_err(native_io_error)?
                    }
                    FileSystemLocation::Directory { directory, path } => {
                        let grant = grants.get(*directory)?;
                        require_write(&grant)?;
                        let fd = call_open(
                            &host,
                            "openWrite",
                            &grant.uri,
                            path.as_str(),
                            Some((request.create_parents, request.overwrite)),
                        )?;
                        unsafe { File::from_raw_fd(fd) }
                    }
                };
                let byte_len = match request.source {
                    FileWriteSource::Bytes(bytes) => {
                        file.write_all(&bytes).map_err(io_error)?;
                        bytes.len() as u64
                    }
                    FileWriteSource::Stream(_) => {
                        let stream = source.as_mut().expect("stream opened above");
                        let mut total = 0_u64;
                        while let Some(chunk) =
                            std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
                        {
                            let chunk = chunk.map_err(|error| {
                                fs_error("stream_read_failed", error.to_string())
                            })?;
                            file.write_all(&chunk).map_err(io_error)?;
                            total = total.saturating_add(chunk.len() as u64);
                        }
                        total
                    }
                };
                file.flush().map_err(io_error)?;
                Ok(WriteFileResult { byte_len })
            }
        },
    );

    let host = context.clone();
    let grants = registry.clone();
    async_registry.register_operation_capability(
        CREATE_DIRECTORY,
        move |request: CreateDirectoryRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                match &request.location {
                    FileSystemLocation::Path(path) => {
                        let result = if request.recursive {
                            std::fs::create_dir_all(path.as_str())
                        } else {
                            std::fs::create_dir(path.as_str())
                        };
                        result.map_err(native_io_error)
                    }
                    FileSystemLocation::Directory { directory, path } => {
                        let grant = grants.get(*directory)?;
                        require_write(&grant)?;
                        call_path_bool(
                            &host,
                            "createDirectory",
                            &grant.uri,
                            path.as_str(),
                            request.recursive,
                        )
                    }
                }
            }
        },
    );

    let host = context;
    let grants = registry.clone();
    async_registry.register_operation_capability(
        REMOVE_ENTRY,
        move |request: RemoveEntryRequest, _| {
            let host = host.clone();
            let grants = grants.clone();
            async move {
                match &request.location {
                    FileSystemLocation::Path(path) => remove_native(path, request.recursive),
                    FileSystemLocation::Directory { directory, path } => {
                        let grant = grants.get(*directory)?;
                        require_write(&grant)?;
                        call_path_bool(
                            &host,
                            "remove",
                            &grant.uri,
                            path.as_str(),
                            request.recursive,
                        )
                    }
                }
            }
        },
    );

    async_registry.register_operation_capability(
        RELEASE_DIRECTORY,
        move |request: ReleaseDirectoryRequest, _| {
            let grants = registry.clone();
            async move { grants.release(request.directory) }
        },
    );
}

fn begin_pick(
    host: &AndroidHostContext,
    id: u64,
    request: &PickDirectoryRequest,
) -> Result<(), FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let key = env.new_string(request.persistence_key.as_deref().unwrap_or_default())?;
        let key = JObject::from(key);
        env.call_static_method(
            helper,
            "beginPick",
            "(Landroid/app/Activity;JZLjava/lang/String;)V",
            &[
                JValue::Object(activity),
                JValue::Long(id as jlong),
                JValue::Bool(matches!(request.access, FileSystemAccessMode::ReadWrite) as u8),
                JValue::Object(&key),
            ],
        )?;
        Ok(())
    })
    .map_err(android_error)
}

fn call_optional_string(
    host: &AndroidHostContext,
    method: &str,
    signature: &str,
    value: &str,
) -> Result<Option<String>, FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let value = JObject::from(env.new_string(value)?);
        let result = env
            .call_static_method(
                helper,
                method,
                signature,
                &[JValue::Object(activity), JValue::Object(&value)],
            )?
            .l()?;
        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(env.get_string(&JString::from(result))?.into()))
        }
    })
    .map_err(android_error)
}

fn call_string(
    host: &AndroidHostContext,
    method: &str,
    signature: &str,
    value: &str,
) -> Result<String, FileSystemError> {
    call_optional_string(host, method, signature, value)?
        .ok_or_else(|| fs_error("android_error", "Android returned no string"))
}

fn call_void_string(
    host: &AndroidHostContext,
    method: &str,
    value: &str,
) -> Result<(), FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let value = JObject::from(env.new_string(value)?);
        env.call_static_method(
            helper,
            method,
            "(Landroid/app/Activity;Ljava/lang/String;)V",
            &[JValue::Object(activity), JValue::Object(&value)],
        )?;
        Ok(())
    })
    .map_err(android_error)
}

fn call_permission(
    host: &AndroidHostContext,
    uri: &str,
    access: FileSystemAccessMode,
) -> Result<bool, FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let uri = JObject::from(env.new_string(uri)?);
        Ok(env
            .call_static_method(
                helper,
                "permission",
                "(Landroid/app/Activity;Ljava/lang/String;Z)I",
                &[
                    JValue::Object(activity),
                    JValue::Object(&uri),
                    JValue::Bool(matches!(access, FileSystemAccessMode::ReadWrite) as u8),
                ],
            )?
            .i()?
            == 1)
    })
    .map_err(android_error)
}

fn call_rows(
    host: &AndroidHostContext,
    method: &str,
    uri: &str,
    path: &str,
) -> Result<Vec<AndroidRow>, FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let uri = JObject::from(env.new_string(uri)?);
        let path = JObject::from(env.new_string(path)?);
        let result = env
            .call_static_method(
                helper,
                method,
                "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)[Ljava/lang/String;",
                &[
                    JValue::Object(activity),
                    JValue::Object(&uri),
                    JValue::Object(&path),
                ],
            )?
            .l()?;
        let result = JObjectArray::from(result);
        let mut rows = Vec::with_capacity(env.get_array_length(&result)? as usize);
        for index in 0..env.get_array_length(&result)? {
            let value = JString::from(env.get_object_array_element(&result, index)?);
            rows.push(
                parse_row(env.get_string(&value)?.to_string_lossy().as_ref())
                    .map_err(|_| jni::errors::Error::NullPtr("invalid filesystem row"))?,
            );
        }
        Ok(rows)
    })
    .map_err(android_error)
}

fn call_row(
    host: &AndroidHostContext,
    method: &str,
    uri: &str,
    path: &str,
) -> Result<AndroidRow, FileSystemError> {
    let value = host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let uri = JObject::from(env.new_string(uri)?);
        let path = JObject::from(env.new_string(path)?);
        let result = env.call_static_method(helper, method, "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(activity), JValue::Object(&uri), JValue::Object(&path)])?.l()?;
        Ok::<String, jni::errors::Error>(env.get_string(&JString::from(result))?.into())
    }).map_err(android_error)?;
    parse_row(&value)
}

fn call_open(
    host: &AndroidHostContext,
    method: &str,
    uri: &str,
    path: &str,
    write: Option<(bool, bool)>,
) -> Result<RawFd, FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let uri = JObject::from(env.new_string(uri)?);
        let path = JObject::from(env.new_string(path)?);
        let (signature, values) = if let Some((parents, overwrite)) = write {
            (
                "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;ZZ)I",
                vec![
                    JValue::Object(activity),
                    JValue::Object(&uri),
                    JValue::Object(&path),
                    JValue::Bool(parents as u8),
                    JValue::Bool(overwrite as u8),
                ],
            )
        } else {
            (
                "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)I",
                vec![
                    JValue::Object(activity),
                    JValue::Object(&uri),
                    JValue::Object(&path),
                ],
            )
        };
        env.call_static_method(helper, method, signature, &values)?
            .i()
    })
    .map_err(android_error)
}

fn call_path_bool(
    host: &AndroidHostContext,
    method: &str,
    uri: &str,
    path: &str,
    value: bool,
) -> Result<(), FileSystemError> {
    host.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let uri = JObject::from(env.new_string(uri)?);
        let path = JObject::from(env.new_string(path)?);
        env.call_static_method(
            helper,
            method,
            "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;Z)V",
            &[
                JValue::Object(activity),
                JValue::Object(&uri),
                JValue::Object(&path),
                JValue::Bool(value as u8),
            ],
        )?;
        Ok(())
    })
    .map_err(android_error)
}

#[derive(Debug)]
struct AndroidRow {
    name: String,
    kind: FileSystemEntryKind,
    byte_len: Option<u64>,
    modified_millis: Option<u64>,
}

impl AndroidRow {
    fn into_entry(self, parent: &FileSystemLocation) -> Result<FileSystemEntry, FileSystemError> {
        let location = parent.join(&self.name);
        self.into_entry_at(location)
    }

    fn into_entry_at(
        self,
        location: FileSystemLocation,
    ) -> Result<FileSystemEntry, FileSystemError> {
        Ok(FileSystemEntry {
            name: self.name,
            location,
            kind: self.kind,
            byte_len: self.byte_len,
            modified_millis: self.modified_millis,
        })
    }
}

fn parse_row(value: &str) -> Result<AndroidRow, FileSystemError> {
    let mut fields = value.split('\0');
    let name = fields.next().unwrap_or_default().to_string();
    let kind = match fields.next().unwrap_or_default() {
        "file" => FileSystemEntryKind::File,
        "directory" => FileSystemEntryKind::Directory,
        _ => FileSystemEntryKind::Other,
    };
    let byte_len = parse_optional_u64(fields.next().unwrap_or_default())?;
    let modified_millis = parse_optional_u64(fields.next().unwrap_or_default())?;
    Ok(AndroidRow {
        name,
        kind,
        byte_len,
        modified_millis,
    })
}

fn parse_optional_u64(value: &str) -> Result<Option<u64>, FileSystemError> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse().map(Some).map_err(|_| {
            fs_error(
                "invalid_host_response",
                "Android returned invalid file metadata",
            )
        })
    }
}

fn access_allows(granted: FileSystemAccessMode, requested: FileSystemAccessMode) -> bool {
    matches!(granted, FileSystemAccessMode::ReadWrite)
        || matches!(requested, FileSystemAccessMode::Read)
}

fn require_write(grant: &AndroidDirectoryGrant) -> Result<(), FileSystemError> {
    if matches!(grant.access, FileSystemAccessMode::ReadWrite) {
        Ok(())
    } else {
        Err(fs_error(
            "permission_denied",
            "the directory was granted for read-only access",
        ))
    }
}

fn android_error(error: String) -> FileSystemError {
    parse_java_error(&error)
}

fn parse_java_error(value: &str) -> FileSystemError {
    if let Some(rest) = value.split("fission_fs:").nth(1) {
        let mut parts = rest.splitn(2, ':');
        return fs_error(
            parts.next().unwrap_or("android_error"),
            parts.next().unwrap_or(rest),
        );
    }
    fs_error("android_error", value)
}

fn io_error(error: std::io::Error) -> FileSystemError {
    fs_error("io_error", error.to_string())
}

fn native_io_error(error: std::io::Error) -> FileSystemError {
    let code = match error.kind() {
        std::io::ErrorKind::NotFound => "not_found",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::AlreadyExists => "already_exists",
        std::io::ErrorKind::InvalidInput => "invalid_path",
        _ => "io_error",
    };
    fs_error(code, error.to_string())
}

fn list_native(
    path: &FileSystemPath,
    parent: &FileSystemLocation,
) -> Result<Vec<FileSystemEntry>, FileSystemError> {
    let native = PathBuf::from(path.as_str());
    std::fs::read_dir(&native)
        .map_err(native_io_error)?
        .map(|entry| {
            let entry = entry.map_err(native_io_error)?;
            let name = entry.file_name().into_string().map_err(|_| {
                fs_error(
                    "unsupported_entry",
                    "the directory contains a name that is not valid UTF-8",
                )
            })?;
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(native_io_error)?;
            Ok(native_entry(name.clone(), parent.join(name), metadata))
        })
        .collect()
}

fn stat_native(
    path: &FileSystemPath,
    location: FileSystemLocation,
) -> Result<FileSystemEntry, FileSystemError> {
    let native = PathBuf::from(path.as_str());
    let metadata = std::fs::metadata(&native).map_err(native_io_error)?;
    let name = native
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| native.to_string_lossy().into_owned());
    Ok(native_entry(name, location, metadata))
}

fn native_entry(name: String, location: FileSystemLocation, metadata: Metadata) -> FileSystemEntry {
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

fn remove_native(path: &FileSystemPath, recursive: bool) -> Result<(), FileSystemError> {
    let native = Path::new(path.as_str());
    let metadata = std::fs::symlink_metadata(native).map_err(native_io_error)?;
    let result = if metadata.is_dir() {
        if recursive {
            std::fs::remove_dir_all(native)
        } else {
            std::fs::remove_dir(native)
        }
    } else {
        std::fs::remove_file(native)
    };
    result.map_err(native_io_error)
}

fn fs_error(code: impl Into<String>, message: impl Into<String>) -> FileSystemError {
    FileSystemError::new(code, message)
}

struct AndroidFileDataStream {
    file: File,
    finished: bool,
}

impl Stream for AndroidFileDataStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        let mut bytes = vec![0; FILE_STREAM_CHUNK_SIZE];
        match self.file.read(&mut bytes) {
            Ok(0) => {
                self.finished = true;
                Poll::Ready(None)
            }
            Ok(read) => {
                bytes.truncate(read);
                Poll::Ready(Some(Ok(Bytes::from(bytes))))
            }
            Err(error) => {
                self.finished = true;
                Poll::Ready(Some(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::Io,
                    error.to_string(),
                ))))
            }
        }
    }
}
