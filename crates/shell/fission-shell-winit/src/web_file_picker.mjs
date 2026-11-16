const pickedFiles = new Map();
const writableFiles = new Map();
let nextPickedFile = 1;
const fallbackFileChunkSize = 256 * 1024;
const maxFileChunkSize = 16 * 1024 * 1024;
const maxBufferedDownloadBytes = 128 * 1024 * 1024;
const savedFileKeyNamespace = "fission.save-file";

function allocateFileId() {
  const first = nextPickedFile;
  do {
    const id = nextPickedFile;
    nextPickedFile = id >= 0xffffffff ? 1 : id + 1;
    if (!pickedFiles.has(id) && !writableFiles.has(id)) return id;
  } while (nextPickedFile !== first);
  throw Object.assign(new Error("too many browser file handles are still active"), { name: "resource_exhausted" });
}

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
  // IndexedDB compound keys cannot collide with the directory API's legacy
  // string keys, even if an app chooses the same visible persistence key.
  return [savedFileKeyNamespace, normalized];
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
  const id = allocateFileId();
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

function safeDownloadName(value) {
  const name = String(value || "download")
    .replace(/[\\/\u0000-\u001f\u007f]/g, "_")
    .trim();
  return name && name !== "." && name !== ".." ? name : "download";
}

function downloadMimeType(types) {
  for (const [, accept] of types) {
    for (const mime of Object.keys(accept || {})) {
      const normalized = mime.toLowerCase();
      if (normalized && !normalized.includes("*")) return normalized;
    }
  }
  return "application/octet-stream";
}

class BrowserDownloadWritable {
  constructor(handle) {
    this.handle = handle;
    this.parts = [];
    this.byteLength = 0;
    this.state = "open";
  }

  write(bytes) {
    if (this.state !== "open") {
      return Promise.reject(Object.assign(new Error("the download writer is no longer open"), { name: "InvalidStateError" }));
    }
    const chunkLength = Number(bytes?.byteLength);
    if (!Number.isSafeInteger(chunkLength) || chunkLength < 0) {
      return Promise.reject(Object.assign(new Error("the download writer requires a byte-array chunk"), { name: "TypeError" }));
    }
    if (this.byteLength + chunkLength > maxBufferedDownloadBytes) {
      return Promise.reject(Object.assign(
        new Error("browser download fallback buffers files in memory and is limited to 128 MiB"),
        { name: "QuotaExceededError" },
      ));
    }
    this.parts.push(bytes);
    this.byteLength += chunkLength;
    return Promise.resolve();
  }

  close() {
    if (this.state !== "open") {
      return Promise.reject(Object.assign(new Error("the download writer is no longer open"), { name: "InvalidStateError" }));
    }
    try {
      // Start the browser download in this same JavaScript turn. A browser may
      // still require transient user activation, and streaming can outlive the
      // activation window, but avoiding another await/timer here gives it the
      // best chance to honor the original export gesture.
      startBrowserDownload(this.handle, this.parts);
      this.state = "closed";
      this.parts = [];
      this.byteLength = 0;
      return Promise.resolve();
    } catch (error) {
      this.state = "errored";
      this.parts = [];
      this.byteLength = 0;
      return Promise.reject(error);
    }
  }

  abort() {
    this.parts = [];
    this.byteLength = 0;
    if (this.state === "open") this.state = "aborted";
    return Promise.resolve();
  }
}

function startBrowserDownload(handle, parts) {
  const urlApi = globalThis.URL;
  if (typeof Blob !== "function"
    || typeof urlApi?.createObjectURL !== "function"
    || typeof urlApi.revokeObjectURL !== "function") {
    throw Object.assign(new Error("browser Blob downloads are unavailable"), { name: "NotSupportedError" });
  }
  const body = globalThis.document?.body;
  if (!body) {
    throw Object.assign(new Error("browser document body is unavailable for the download"), { name: "InvalidStateError" });
  }

  const anchor = document.createElement("a");
  const url = urlApi.createObjectURL(new Blob(parts, { type: handle.type }));
  const revokeUrl = urlApi.revokeObjectURL.bind(urlApi);
  anchor.href = url;
  anchor.download = handle.name;
  anchor.style.display = "none";
  try {
    body.appendChild(anchor);
    anchor.click();
  } finally {
    anchor.remove();
    // Keep the object URL alive through the browser's download dispatch.
    globalThis.setTimeout(() => revokeUrl(url), 1000);
  }
}

export function fissionPickSaveFile(suggestedName, serializedTypes, persistenceKey = "") {
  return (async () => {
    const types = JSON.parse(serializedTypes);
    const options = { suggestedName: suggestedName || "download" };
    if (types.length > 0) {
      options.types = types.map(([description, accept]) => ({ description, accept }));
    }
    if (typeof globalThis.showSaveFilePicker !== "function") {
      // The fallback is an ordinary browser download, not a persistent file
      // handle or a Save dialog. The browser owns the final location and does
      // not report whether a later download was cancelled by the user.
      const id = allocateFileId();
      const handle = {
        fissionDownload: true,
        id,
        name: safeDownloadName(options.suggestedName),
        type: downloadMimeType(types),
      };
      writableFiles.set(id, handle);
      return { id, name: handle.name, permission: "unknown" };
    }

    let handle;
    try {
      handle = await globalThis.showSaveFilePicker(options);
    } catch (error) {
      if (error && error.name === "AbortError") return null;
      throw error;
    }
    if (persistenceKey) await storedSaveFileHandle(requireSaveFileKey(persistenceKey), handle, false);
    const id = allocateFileId();
    writableFiles.set(id, handle);
    return { id, name: handle.name || options.suggestedName, permission: await permission(handle, false) };
  })();
}

export async function fissionRestoreSaveFile(persistenceKey) {
  const key = requireSaveFileKey(persistenceKey);
  const handle = await storedSaveFileHandle(key, undefined, false);
  if (!handle) return null;
  const id = allocateFileId();
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
  if (handle.fissionDownload) return new BrowserDownloadWritable(handle);
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

export function fissionReleaseWritableFile(fileId) {
  writableFiles.delete(Number(fileId));
}
