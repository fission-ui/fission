const pickedFiles = new Map();
const writableFiles = new Map();
let nextPickedFile = 1;
const fallbackFileChunkSize = 256 * 1024;
const maxFileChunkSize = 16 * 1024 * 1024;
const savedFileKeyPrefix = "save-file:";

function permissionName(value) {
  return value === "granted" || value === "prompt" || value === "denied" ? value : "unknown";
}

async function permission(handle, request) {
  const options = { mode: "readwrite" };
  if (request && handle.requestPermission) return permissionName(await handle.requestPermission(options));
  if (handle.queryPermission) return permissionName(await handle.queryPermission(options));
  return "unknown";
}

function requireSaveFileKey(key) {
  const normalized = String(key || "").trim();
  if (!normalized) throw Object.assign(new Error("save-file persistence keys cannot be empty"), { name: "invalid_persistence_key" });
  return `${savedFileKeyPrefix}${normalized}`;
}

function openHandleDatabase() {
    if (!globalThis.indexedDB) throw Object.assign(new Error("IndexedDB is unavailable, so save-file handles cannot be persisted"), { name: "unsupported" });
  return new Promise((resolve, reject) => {
    const request = indexedDB.open("fission-file-system-handles", 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains("handles")) request.result.createObjectStore("handles");
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error("failed to open the save-file handle database"));
  });
}

async function storedSaveFileHandle(key, value, remove) {
  const database = await openHandleDatabase();
  try {
    return await new Promise((resolve, reject) => {
      const transaction = database.transaction("handles", remove || value !== undefined ? "readwrite" : "readonly");
      const store = transaction.objectStore("handles");
      const request = remove ? store.delete(key) : (value !== undefined ? store.put(value, key) : store.get(key));
      let result;
      request.onsuccess = () => { result = request.result; };
      request.onerror = () => reject(request.error || new Error("save-file handle persistence failed"));
      transaction.oncomplete = () => resolve(result);
      transaction.onerror = () => reject(transaction.error || new Error("save-file handle transaction failed"));
      transaction.onabort = () => reject(transaction.error || new Error("save-file handle transaction was aborted"));
    });
  } finally {
    database.close();
  }
}

function registerPickedFile(file) {
  const id = nextPickedFile;
  nextPickedFile = id >= 0xffffffff ? 1 : id + 1;
  if (pickedFiles.has(id)) throw Object.assign(new Error("too many selected files are still active"), { name: "resource_exhausted" });
  pickedFiles.set(id, file);
  return id;
}

export function fissionPickOpenFiles(allowMultiple, accept) {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = Boolean(allowMultiple);
    if (accept) input.accept = accept;
    input.style.position = "fixed";
    input.style.left = "-10000px";
    input.style.width = "1px";
    input.style.height = "1px";
    document.body.appendChild(input);

    let settled = false;
    const finish = (value, error) => {
      if (settled) return;
      settled = true;
      input.remove();
      if (error) reject(error); else resolve(value);
    };
    input.oncancel = () => finish([]);
    input.onchange = () => {
      try {
        const selected = [];
        for (const file of Array.from(input.files || [])) {
          selected.push({
            name: file.name || "selected-file",
            contentType: file.type || null,
            byteLen: file.size,
            fileId: registerPickedFile(file),
          });
        }
        finish(selected);
      } catch (error) {
        finish(null, error);
      }
    };
    input.click();
  });
}

export function fissionOpenPickedFileReader(fileId, preferredChunkBytes) {
  const id = Number(fileId);
  const file = pickedFiles.get(id);
  if (!file) throw Object.assign(new Error("the selected browser file is no longer available"), { name: "invalid_handle" });
  pickedFiles.delete(id);

  const requestedChunkSize = Number(preferredChunkBytes);
  if (!(Number.isSafeInteger(requestedChunkSize) && requestedChunkSize > 0) && typeof file.stream === "function") {
    const reader = file.stream().getReader();
    let pending = null;
    let offset = 0;
    return {
      async read() {
        while (!pending || offset >= pending.byteLength) {
          const result = await reader.read();
          if (result.done) return { done: true, value: undefined };
          pending = result.value instanceof Uint8Array ? result.value : new Uint8Array(result.value);
          offset = 0;
          if (pending.byteLength === 0) continue;
        }
        const end = Math.min(offset + maxFileChunkSize, pending.byteLength);
        const value = pending.subarray(offset, end);
        offset = end;
        return { done: false, value };
      },
      cancel() { return reader.cancel().catch(() => undefined); },
    };
  }

  const chunkSize = Number.isSafeInteger(requestedChunkSize) && requestedChunkSize > 0
    ? Math.min(requestedChunkSize, maxFileChunkSize)
    : fallbackFileChunkSize;
  let offset = 0;
  let cancelled = false;
  return {
    async read() {
      if (cancelled || offset >= file.size) return { done: true, value: undefined };
      const end = Math.min(offset + chunkSize, file.size);
      const value = new Uint8Array(await file.slice(offset, end).arrayBuffer());
      offset = end;
      return { done: false, value };
    },
    async cancel() { cancelled = true; },
  };
}

export function fissionReadPickedFileChunk(reader) {
  return reader.read();
}

export function fissionCancelPickedFileReader(reader) {
  return reader.cancel().catch(() => undefined);
}

export function fissionReleasePickedFile(fileId) {
  pickedFiles.delete(Number(fileId));
}

export function fissionPickSaveFile(suggestedName, serializedTypes, persistenceKey = "") {
  return (async () => {
    if (!("showSaveFilePicker" in globalThis)) {
      throw Object.assign(new Error("the browser save-file picker is unavailable"), { name: "unsupported" });
    }
    const types = JSON.parse(serializedTypes);
    const options = { suggestedName: suggestedName || "download" };
    if (types.length > 0) {
      options.types = types.map(([description, accept]) => ({ description, accept }));
    }
    let handle;
    try {
      handle = await globalThis.showSaveFilePicker(options);
    } catch (error) {
      if (error && error.name === "AbortError") return null;
      throw error;
    }
    if (persistenceKey) await storedSaveFileHandle(requireSaveFileKey(persistenceKey), handle, false);
    const id = nextPickedFile;
    nextPickedFile = id >= 0xffffffff ? 1 : id + 1;
    if (writableFiles.has(id)) throw Object.assign(new Error("too many save destinations are still active"), { name: "resource_exhausted" });
    writableFiles.set(id, handle);
    return { id, name: handle.name || options.suggestedName, permission: await permission(handle, false) };
  })();
}

export async function fissionRestoreSaveFile(persistenceKey) {
  const key = requireSaveFileKey(persistenceKey);
  const handle = await storedSaveFileHandle(key, undefined, false);
  if (!handle) return null;
  const id = nextPickedFile;
  nextPickedFile = id >= 0xffffffff ? 1 : id + 1;
  if (writableFiles.has(id)) throw Object.assign(new Error("too many save destinations are still active"), { name: "resource_exhausted" });
  writableFiles.set(id, handle);
  return { id, name: handle.name || "selected-file", permission: await permission(handle, false) };
}

export async function fissionForgetSaveFile(persistenceKey) {
  await storedSaveFileHandle(requireSaveFileKey(persistenceKey), undefined, true);
}

export async function fissionWritableFilePermission(fileId, request = false) {
  const handle = writableFiles.get(Number(fileId));
  if (!handle) throw Object.assign(new Error("the selected browser save destination is no longer active"), { name: "invalid_handle" });
  return permission(handle, Boolean(request));
}

export async function fissionOpenPickedFileWriter(fileId) {
  const id = Number(fileId);
  const handle = writableFiles.get(id);
  if (!handle) throw Object.assign(new Error("the selected browser save destination is no longer active"), { name: "invalid_handle" });
  return handle.createWritable();
}

export function fissionWritePickedFileChunk(writable, bytes) {
  return writable.write(bytes);
}

export function fissionClosePickedFileWriter(writable) {
  return writable.close();
}

export function fissionAbortPickedFileWriter(writable) {
  return writable.abort();
}
