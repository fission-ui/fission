use fission_core::{
    collect_data_stream, single_chunk_data_stream, CreateDirectoryRequest, DirectoryHandle,
    DirectoryHandleId, DirectoryPermissionRequest, DirectoryPermissionResult, FileSystemAccessMode,
    FileSystemEntry, FileSystemEntryKind, FileSystemError, FileSystemPath, FileSystemPermission,
    FileWriteSource, ForgetDirectoryRequest, ListDirectoryRequest, ListDirectoryResult,
    PickDirectoryRequest, PickDirectoryResult, ReadFileRequest, ReadFileResult,
    ReleaseDirectoryRequest, RemoveEntryRequest, RestoreDirectoryRequest, RestoreDirectoryResult,
    StatEntryRequest, StatEntryResult, WriteFileRequest, WriteFileResult, CREATE_DIRECTORY,
    DIRECTORY_PERMISSION, FORGET_DIRECTORY, LIST_DIRECTORY, PICK_DIRECTORY, READ_FILE,
    RELEASE_DIRECTORY, REMOVE_ENTRY, RESTORE_DIRECTORY, STAT_ENTRY, WRITE_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use js_sys::{Array, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(inline_js = r#"
const handles = new Map();
let nextHandle = 1;

function unsupported(message) {
  const error = new Error(message);
  error.name = "unsupported";
  return error;
}

function requireHandle(id) {
  const handle = handles.get(Number(id));
  if (!handle) throw Object.assign(new Error("the directory handle is no longer active"), { name: "invalid_handle" });
  return handle;
}

function registerHandle(handle) {
  const id = nextHandle++;
  handles.set(id, handle);
  return id;
}

function parts(path) {
  return path ? String(path).split("/") : [];
}

async function directoryAt(root, path, create) {
  let current = root;
  for (const component of parts(path)) {
    current = await current.getDirectoryHandle(component, { create: Boolean(create) });
  }
  return current;
}

async function parentAndName(root, path, createParents) {
  const components = parts(path);
  if (components.length === 0) throw Object.assign(new Error("the directory root is not a file entry"), { name: "invalid_path" });
  const name = components.pop();
  return { parent: await directoryAt(root, components.join("/"), createParents), name };
}

function permissionName(value) {
  return value === "granted" || value === "prompt" || value === "denied" ? value : "unknown";
}

async function permission(handle, mode, request) {
  const options = { mode: mode === "readwrite" ? "readwrite" : "read" };
  if (request && handle.requestPermission) return permissionName(await handle.requestPermission(options));
  if (handle.queryPermission) return permissionName(await handle.queryPermission(options));
  return "unknown";
}

function openDatabase() {
  if (!("indexedDB" in globalThis)) throw unsupported("IndexedDB is unavailable, so directory handles cannot be persisted");
  return new Promise((resolve, reject) => {
    const request = indexedDB.open("fission-file-system-handles", 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains("handles")) request.result.createObjectStore("handles");
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error("failed to open the directory-handle database"));
  });
}

async function storedHandle(key, value, remove) {
  const database = await openDatabase();
  try {
    return await new Promise((resolve, reject) => {
      const transaction = database.transaction("handles", remove || value !== undefined ? "readwrite" : "readonly");
      const store = transaction.objectStore("handles");
      const request = remove ? store.delete(key) : (value !== undefined ? store.put(value, key) : store.get(key));
      let result;
      request.onsuccess = () => { result = request.result; };
      request.onerror = () => reject(request.error || new Error("directory-handle persistence failed"));
      transaction.oncomplete = () => resolve(result);
      transaction.onerror = () => reject(transaction.error || new Error("directory-handle transaction failed"));
      transaction.onabort = () => reject(transaction.error || new Error("directory-handle transaction was aborted"));
    });
  } finally {
    database.close();
  }
}

export function fissionPickDirectory(mode, persistenceKey) {
  return (async () => {
    if (!("showDirectoryPicker" in globalThis)) throw unsupported("the File System Access directory picker is unavailable in this browser");
    try {
      const handle = await globalThis.showDirectoryPicker({ mode: mode === "readwrite" ? "readwrite" : "read" });
      if (persistenceKey) await storedHandle(persistenceKey, handle, false);
      return { id: registerHandle(handle), name: handle.name || "selected-folder", permission: await permission(handle, mode, false) };
    } catch (error) {
      if (error && error.name === "AbortError") return null;
      throw error;
    }
  })();
}

export function fissionRestoreDirectory(mode, persistenceKey) {
  return (async () => {
    const handle = await storedHandle(persistenceKey, undefined, false);
    if (!handle) return null;
    return { id: registerHandle(handle), name: handle.name || "selected-folder", permission: await permission(handle, mode, false) };
  })();
}

export function fissionForgetDirectory(persistenceKey) {
  return storedHandle(persistenceKey, undefined, true).then(() => undefined);
}

export function fissionDirectoryPermission(id, mode, request) {
  return permission(requireHandle(id), mode, request);
}

export function fissionListDirectory(id, path) {
  return (async () => {
    const directory = await directoryAt(requireHandle(id), path, false);
    const entries = [];
    for await (const [name, handle] of directory.entries()) {
      const childPath = path ? `${path}/${name}` : name;
      if (handle.kind === "file") {
        const file = await handle.getFile();
        entries.push({ name, path: childPath, kind: "file", byteLen: file.size, modifiedMillis: file.lastModified });
      } else {
        entries.push({ name, path: childPath, kind: "directory", byteLen: null, modifiedMillis: null });
      }
    }
    entries.sort((left, right) => left.name.localeCompare(right.name));
    return entries;
  })();
}

export function fissionStatEntry(id, path) {
  return (async () => {
    const root = requireHandle(id);
    if (!path) return { name: root.name || "selected-folder", path: "", kind: "directory", byteLen: null, modifiedMillis: null };
    const { parent, name } = await parentAndName(root, path, false);
    try {
      const file = await (await parent.getFileHandle(name)).getFile();
      return { name, path, kind: "file", byteLen: file.size, modifiedMillis: file.lastModified };
    } catch (error) {
      try {
        await parent.getDirectoryHandle(name);
        return { name, path, kind: "directory", byteLen: null, modifiedMillis: null };
      } catch (_) {}
      throw error;
    }
  })();
}

export function fissionReadFile(id, path) {
  return (async () => {
    const { parent, name } = await parentAndName(requireHandle(id), path, false);
    const file = await (await parent.getFileHandle(name)).getFile();
    return { bytes: new Uint8Array(await file.arrayBuffer()), byteLen: file.size, modifiedMillis: file.lastModified };
  })();
}

export function fissionWriteFile(id, path, bytes, createParents, overwrite) {
  return (async () => {
    const { parent, name } = await parentAndName(requireHandle(id), path, createParents);
    if (!overwrite) {
      try {
        await parent.getFileHandle(name);
        throw Object.assign(new Error("the file already exists"), { name: "already_exists" });
      } catch (error) {
        if (!error || error.name !== "NotFoundError") throw error;
      }
    }
    const writable = await (await parent.getFileHandle(name, { create: true })).createWritable();
    try {
      await writable.write(bytes);
      await writable.close();
      return bytes.byteLength;
    } catch (error) {
      try { await writable.abort(); } catch (_) {}
      throw error;
    }
  })();
}

export function fissionCreateDirectory(id, path, recursive) {
  return (async () => {
    const root = requireHandle(id);
    if (!path) return;
    if (recursive) return void await directoryAt(root, path, true);
    const { parent, name } = await parentAndName(root, path, false);
    await parent.getDirectoryHandle(name, { create: true });
  })();
}

export function fissionRemoveEntry(id, path, recursive) {
  return (async () => {
    const { parent, name } = await parentAndName(requireHandle(id), path, false);
    await parent.removeEntry(name, { recursive: Boolean(recursive) });
  })();
}

export function fissionReleaseDirectory(id) {
  if (!handles.delete(Number(id))) throw Object.assign(new Error("the directory handle is no longer active"), { name: "invalid_handle" });
  return Promise.resolve();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn fissionPickDirectory(mode: &str, persistence_key: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionRestoreDirectory(mode: &str, persistence_key: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionForgetDirectory(persistence_key: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionDirectoryPermission(id: u32, mode: &str, request: bool) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionListDirectory(id: u32, path: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionStatEntry(id: u32, path: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionReadFile(id: u32, path: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionWriteFile(
        id: u32,
        path: &str,
        bytes: &Uint8Array,
        create_parents: bool,
        overwrite: bool,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCreateDirectory(id: u32, path: &str, recursive: bool) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionRemoveEntry(id: u32, path: &str, recursive: bool) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionReleaseDirectory(id: u32) -> Result<Promise, JsValue>;
}

pub(crate) fn register_web_file_system_capabilities(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PICK_DIRECTORY,
        |request: PickDirectoryRequest, _| async move {
            if let Some(key) = request.persistence_key.as_deref() {
                require_key(key)?;
            }
            let value = await_promise(fissionPickDirectory(
                mode(request.access),
                request.persistence_key.as_deref().unwrap_or(""),
            ))
            .await
            .map_err(file_system_error)?;
            Ok(PickDirectoryResult {
                directory: directory_handle(&value, request.access)?,
            })
        },
    );
    async_registry.register_operation_capability(
        RESTORE_DIRECTORY,
        |request: RestoreDirectoryRequest, _| async move {
            require_key(&request.persistence_key)?;
            let value = await_promise(fissionRestoreDirectory(
                mode(request.access),
                &request.persistence_key,
            ))
            .await
            .map_err(file_system_error)?;
            Ok(RestoreDirectoryResult {
                directory: directory_handle(&value, request.access)?,
            })
        },
    );
    async_registry.register_operation_capability(
        FORGET_DIRECTORY,
        |request: ForgetDirectoryRequest, _| async move {
            require_key(&request.persistence_key)?;
            await_promise(fissionForgetDirectory(&request.persistence_key))
                .await
                .map_err(file_system_error)?;
            Ok(())
        },
    );
    async_registry.register_operation_capability(
        DIRECTORY_PERMISSION,
        |request: DirectoryPermissionRequest, _| async move {
            let value = await_promise(fissionDirectoryPermission(
                directory_id(request.directory)?,
                mode(request.access),
                request.request,
            ))
            .await
            .map_err(file_system_error)?;
            Ok(DirectoryPermissionResult {
                permission: permission_value(value.as_string().as_deref()),
            })
        },
    );
    async_registry.register_operation_capability(
        LIST_DIRECTORY,
        |request: ListDirectoryRequest, _| async move {
            let value = await_promise(fissionListDirectory(
                directory_id(request.directory)?,
                request.path.as_str(),
            ))
            .await
            .map_err(file_system_error)?;
            let values = value.dyn_into::<Array>().map_err(|_| {
                FileSystemError::new(
                    "invalid_result",
                    "browser directory listing was not an array",
                )
            })?;
            Ok(ListDirectoryResult {
                entries: values
                    .iter()
                    .map(|value| entry(&value))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        },
    );
    async_registry.register_operation_capability(
        STAT_ENTRY,
        |request: StatEntryRequest, _| async move {
            let value = await_promise(fissionStatEntry(
                directory_id(request.directory)?,
                request.path.as_str(),
            ))
            .await
            .map_err(file_system_error)?;
            Ok(StatEntryResult {
                entry: entry(&value)?,
            })
        },
    );
    async_registry.register_operation_capability(
        READ_FILE,
        |request: ReadFileRequest, ctx| async move {
            let value = await_promise(fissionReadFile(
                directory_id(request.directory)?,
                request.path.as_str(),
            ))
            .await
            .map_err(file_system_error)?;
            let bytes = prop(&value, "bytes")
                .and_then(|value| value.dyn_into::<Uint8Array>().ok())
                .ok_or_else(|| {
                    FileSystemError::new("invalid_result", "browser file read returned no bytes")
                })?
                .to_vec();
            Ok(ReadFileResult {
                stream: ctx.register_data_stream(single_chunk_data_stream(bytes)),
                byte_len: u64_prop(&value, "byteLen"),
                modified_millis: u64_prop(&value, "modifiedMillis"),
            })
        },
    );
    async_registry.register_operation_capability(
        WRITE_FILE,
        |request: WriteFileRequest, ctx| async move {
            let bytes = match request.source {
                FileWriteSource::Bytes(bytes) => bytes,
                FileWriteSource::Stream(id) => {
                    let stream = ctx.open_data_stream(id).map_err(|error| {
                        FileSystemError::new("stream_open_failed", error.to_string())
                    })?;
                    collect_data_stream(stream)
                        .await
                        .map_err(|error| {
                            FileSystemError::new("stream_read_failed", error.to_string())
                        })?
                        .to_vec()
                }
            };
            let value = await_promise(fissionWriteFile(
                directory_id(request.directory)?,
                request.path.as_str(),
                &Uint8Array::from(bytes.as_slice()),
                request.create_parents,
                request.overwrite,
            ))
            .await
            .map_err(file_system_error)?;
            Ok(WriteFileResult {
                byte_len: value.as_f64().unwrap_or(bytes.len() as f64).max(0.0) as u64,
            })
        },
    );
    async_registry.register_operation_capability(
        CREATE_DIRECTORY,
        |request: CreateDirectoryRequest, _| async move {
            await_promise(fissionCreateDirectory(
                directory_id(request.directory)?,
                request.path.as_str(),
                request.recursive,
            ))
            .await
            .map_err(file_system_error)?;
            Ok(())
        },
    );
    async_registry.register_operation_capability(
        REMOVE_ENTRY,
        |request: RemoveEntryRequest, _| async move {
            if request.path.is_root() {
                return Err(FileSystemError::new(
                    "invalid_path",
                    "the granted directory itself cannot be removed",
                ));
            }
            await_promise(fissionRemoveEntry(
                directory_id(request.directory)?,
                request.path.as_str(),
                request.recursive,
            ))
            .await
            .map_err(file_system_error)?;
            Ok(())
        },
    );
    async_registry.register_operation_capability(
        RELEASE_DIRECTORY,
        |request: ReleaseDirectoryRequest, _| async move {
            await_promise(fissionReleaseDirectory(directory_id(request.directory)?))
                .await
                .map_err(file_system_error)?;
            Ok(())
        },
    );
}

fn mode(access: FileSystemAccessMode) -> &'static str {
    match access {
        FileSystemAccessMode::Read => "read",
        FileSystemAccessMode::ReadWrite => "readwrite",
    }
}

fn directory_id(id: DirectoryHandleId) -> Result<u32, FileSystemError> {
    u32::try_from(id.0).map_err(|_| {
        FileSystemError::new(
            "invalid_handle",
            "the directory handle is invalid for this host",
        )
    })
}

fn require_key(key: &str) -> Result<(), FileSystemError> {
    if key.trim().is_empty() {
        Err(FileSystemError::new(
            "invalid_persistence_key",
            "directory persistence keys cannot be empty",
        ))
    } else {
        Ok(())
    }
}

fn directory_handle(
    value: &JsValue,
    access: FileSystemAccessMode,
) -> Result<Option<DirectoryHandle>, FileSystemError> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let id = f64_prop(value, "id")
        .filter(|id| id.is_finite() && *id > 0.0 && *id <= u32::MAX as f64)
        .ok_or_else(|| {
            FileSystemError::new(
                "invalid_result",
                "browser returned an invalid directory handle",
            )
        })?;
    Ok(Some(DirectoryHandle {
        id: DirectoryHandleId(id as u64),
        name: string_prop(value, "name").unwrap_or_else(|| "selected-folder".into()),
        access,
        permission: permission_value(string_prop(value, "permission").as_deref()),
    }))
}

fn permission_value(permission: Option<&str>) -> FileSystemPermission {
    match permission {
        Some("granted") => FileSystemPermission::Granted,
        Some("prompt") => FileSystemPermission::Prompt,
        Some("denied") => FileSystemPermission::Denied,
        _ => FileSystemPermission::Unknown,
    }
}

fn entry(value: &JsValue) -> Result<FileSystemEntry, FileSystemError> {
    let path = string_prop(value, "path")
        .ok_or_else(|| FileSystemError::new("invalid_result", "browser entry omitted its path"))?;
    let kind = match string_prop(value, "kind").as_deref() {
        Some("file") => FileSystemEntryKind::File,
        Some("directory") => FileSystemEntryKind::Directory,
        _ => FileSystemEntryKind::Other,
    };
    Ok(FileSystemEntry {
        name: string_prop(value, "name").unwrap_or_default(),
        path: FileSystemPath::new(path)
            .map_err(|error| FileSystemError::new("invalid_result", error.to_string()))?,
        kind,
        byte_len: u64_prop(value, "byteLen"),
        modified_millis: u64_prop(value, "modifiedMillis"),
    })
}

async fn await_promise(result: Result<Promise, JsValue>) -> Result<JsValue, JsValue> {
    JsFuture::from(result?).await
}

fn prop(value: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .filter(|value| !value.is_null() && !value.is_undefined())
}

fn string_prop(value: &JsValue, name: &str) -> Option<String> {
    prop(value, name).and_then(|value| value.as_string())
}

fn f64_prop(value: &JsValue, name: &str) -> Option<f64> {
    prop(value, name).and_then(|value| value.as_f64())
}

fn u64_prop(value: &JsValue, name: &str) -> Option<u64> {
    f64_prop(value, name)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.min(u64::MAX as f64) as u64)
}

fn file_system_error(value: JsValue) -> FileSystemError {
    let raw_code = string_prop(&value, "name")
        .unwrap_or_else(|| "host_error".into())
        .to_ascii_lowercase();
    let message = string_prop(&value, "message")
        .or_else(|| value.as_string())
        .unwrap_or_else(|| format!("{value:?}"));
    let code = match raw_code.as_str() {
        "notallowederror" | "nomodificationallowederror" => "permission_denied",
        "notfounderror" => "not_found",
        "typemismatcherror" => "wrong_entry_type",
        "invalidstateerror" => "invalid_handle",
        "securityerror" => "unavailable_in_context",
        "aborterror" => "cancelled",
        "quotaexceedederror" => "quota_exceeded",
        other => other,
    };
    FileSystemError::new(code, message)
}
