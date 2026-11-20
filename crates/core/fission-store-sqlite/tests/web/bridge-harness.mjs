// Protocol/lifetime tests run the real bridge and worker modules. SQLite's
// storage primitives and browser transports are simulated; the Chromium
// fixture separately checks real SQLite, OPFS and Web Locks.
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import { webcrypto } from "node:crypto";

const bridgeSource = await readFile(new URL("../../assets/web/fission-sqlite.mjs", import.meta.url), "utf8");
const workerSource = await readFile(new URL("../../assets/web/fission-sqlite-worker.mjs", import.meta.url), "utf8");

function abortError() {
  const error = new Error("The context's lock request was aborted");
  error.name = "AbortError";
  return error;
}

function notFound() {
  const error = new Error("OPFS entry was not found");
  error.name = "NotFoundError";
  return error;
}

function createOpfs(beforeAccess) {
  const directories = new Set([""]);
  const files = new Map();
  const normalize = (path) => path.replace(/^\/+/, "");
  const pathJoin = (parent, name) => parent ? `${parent}/${name}` : name;
  const ensureFile = (path) => {
    path = normalize(path);
    if (!files.has(path)) files.set(path, new Uint8Array());
    return files.get(path);
  };
  const replaceFile = (path, bytes) => files.set(normalize(path), new Uint8Array(bytes));
  function directory(path) {
    return {
      kind: "directory",
      name: path.split("/").at(-1) || "",
      async getDirectoryHandle(name, options = {}) {
        const child = pathJoin(path, name);
        if (!directories.has(child)) {
          if (!options.create) throw notFound();
          directories.add(child);
        }
        return directory(child);
      },
      async getFileHandle(name, options = {}) {
        const child = pathJoin(path, name);
        if (!files.has(child)) {
          if (!options.create) throw notFound();
          ensureFile(child);
        }
        return {
          kind: "file",
          name,
          async createSyncAccessHandle() {
            await beforeAccess?.(child);
            let closed = false;
            return {
              getSize() { return ensureFile(child).length; },
              read(target, { at = 0 } = {}) {
                if (closed) throw new Error("access handle is closed");
                const source = ensureFile(child);
                const count = Math.max(0, Math.min(target.length, source.length - at));
                target.set(source.subarray(at, at + count));
                return count;
              },
              write(source, { at = 0 } = {}) {
                if (closed) throw new Error("access handle is closed");
                const previous = ensureFile(child);
                const next = new Uint8Array(Math.max(previous.length, at + source.length));
                next.set(previous);
                next.set(source, at);
                replaceFile(child, next);
                return source.length;
              },
              truncate(size) { replaceFile(child, ensureFile(child).slice(0, size)); },
              flush() {},
              close() { closed = true; },
            };
          },
        };
      },
      async removeEntry(name, { recursive = false } = {}) {
        const child = pathJoin(path, name);
        if (files.delete(child)) return;
        if (!directories.has(child)) throw notFound();
        const descendantFiles = [...files.keys()].filter((entry) => entry.startsWith(`${child}/`));
        const descendantDirectories = [...directories].filter((entry) => entry.startsWith(`${child}/`));
        if (!recursive && (descendantFiles.length || descendantDirectories.length)) {
          throw new Error("directory is not empty");
        }
        for (const entry of descendantFiles) files.delete(entry);
        for (const entry of descendantDirectories) directories.delete(entry);
        directories.delete(child);
      },
      async *entries() {
        const prefix = path ? `${path}/` : "";
        const children = new Map();
        for (const entry of directories) {
          if (!entry.startsWith(prefix) || entry === path) continue;
          const name = entry.slice(prefix.length).split("/")[0];
          children.set(name, directory(pathJoin(path, name)));
        }
        for (const entry of files.keys()) {
          if (!entry.startsWith(prefix)) continue;
          const name = entry.slice(prefix.length).split("/")[0];
          children.set(name, { kind: "file", name });
        }
        for (const item of children) yield item;
      },
    };
  }
  return {
    storage: { getDirectory: async () => directory("") },
    ensureFile,
    replaceFile,
    readFile: (path) => ensureFile(path),
    listFiles: (prefix = "") => [...files.keys()].filter((path) => path.startsWith(normalize(prefix))),
  };
}

function cloneDatabaseState(state) {
  return { marker: state.marker, payload: new Uint8Array(state.payload), integrity: state.integrity };
}

function encodeDatabaseState(state) {
  const marker = new TextEncoder().encode(state.marker);
  const bytes = new Uint8Array(8 + marker.length + state.payload.length);
  bytes.set([70, 83, 81, 76]); // FSQL
  new DataView(bytes.buffer).setUint32(4, marker.length);
  bytes.set(marker, 8);
  bytes.set(state.payload, 8 + marker.length);
  return bytes;
}

function decodeDatabaseState(bytes) {
  if (bytes.length < 8 || bytes[0] !== 70 || bytes[1] !== 83 || bytes[2] !== 81 || bytes[3] !== 76) {
    throw new Error("malformed SQLite fixture");
  }
  const markerLength = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(4);
  if (8 + markerLength > bytes.length) throw new Error("malformed SQLite fixture");
  return {
    marker: new TextDecoder().decode(bytes.subarray(8, 8 + markerLength)),
    payload: bytes.slice(8 + markerLength),
    integrity: "ok",
  };
}

class LockService {
  locks = new Map();

  forContext(context) {
    return { request: (name, options, callback) => this.request(context, name, options, callback) };
  }

  request(context, name, options, callback) {
    if (typeof options === "function") [callback, options] = [options, {}];
    const lock = this.locks.get(name) || { active: new Set(), pending: [] };
    this.locks.set(name, lock);
    return new Promise((resolve, reject) => {
      const entry = { context, mode: options.mode || "exclusive", callback, resolve, reject, done: false };
      if (options.signal?.aborted) return reject(abortError());
      lock.pending.push(entry);
      options.signal?.addEventListener("abort", () => {
        const index = lock.pending.indexOf(entry);
        // The platform cannot revoke an already granted lock via AbortSignal.
        if (index < 0) return;
        lock.pending.splice(index, 1);
        entry.reject(abortError());
        this.pump(lock);
      }, { once: true });
      this.pump(lock);
    });
  }

  pump(lock) {
    while (lock.pending.length) {
      const first = lock.pending[0];
      if ([...lock.active].some((entry) => entry.mode === "exclusive")) return;
      if (first.mode === "exclusive" && lock.active.size) return;
      const entry = lock.pending.shift();
      lock.active.add(entry);
      Promise.resolve().then(entry.callback).then(entry.resolve, entry.reject)
        .finally(() => this.release(lock, entry));
      if (entry.mode === "exclusive") return;
    }
  }

  release(lock, entry) {
    if (entry.done) return;
    entry.done = true;
    lock.active.delete(entry);
    this.pump(lock);
  }

  destroy(context) {
    for (const lock of this.locks.values()) {
      for (const entry of [...lock.pending]) {
        if (entry.context !== context) continue;
        lock.pending.splice(lock.pending.indexOf(entry), 1);
        entry.reject(abortError());
      }
      // Browser destruction releases granted locks even though the callback's
      // JavaScript promise can no longer run to completion.
      for (const entry of [...lock.active]) {
        if (entry.context === context) this.release(lock, entry);
      }
      this.pump(lock);
    }
  }
}

function moduleOptions(context, identifier) {
  return { context, identifier, initializeImportMeta(meta, module) { meta.url = module.identifier; } };
}

export function createHarness(options = {}) {
  const channels = new Map();
  const locks = new LockService();
  const opfs = createOpfs(options.beforeOpfsAccess);
  const workers = [];
  const pages = [];

  class Channel {
    constructor(name, owner) {
      this.name = name;
      this.owner = owner;
      this.listeners = [];
      this.closed = false;
      if (!channels.has(name)) channels.set(name, new Set());
      channels.get(name).add(this);
    }
    addEventListener(type, callback) { if (type === "message") this.listeners.push(callback); }
    postMessage(data) {
      for (const other of channels.get(this.name)) {
        if (other === this || other.closed) continue;
        queueMicrotask(() => {
          if (!other.closed) other.listeners.forEach((callback) => callback({ data: structuredClone(data) }));
        });
      }
    }
    close() { this.closed = true; channels.get(this.name).delete(this); }
  }

  class Worker {
    constructor(url, owner) {
      if (options.workerConstructionFailure) throw new Error("worker construction denied");
      this.owner = owner;
      this.listeners = new Map();
      this.terminated = false;
      this.commands = [];
      workers.push(this);
      if (options.workerStartupFailure) {
        queueMicrotask(() => {
          this.listeners.get("error")?.forEach((callback) => callback({ message: "worker failed before ready" }));
        });
        return;
      }
      const worker = this;
      const endpoint = {
        addEventListener(type, callback) { if (type === "message") worker.onMessage = callback; },
        postMessage(data) {
          queueMicrotask(() => {
            if (!worker.terminated) worker.listeners.get("message")?.forEach((callback) => callback({ data }));
          });
        },
      };
      let authorizer;
      class Database {
        constructor() { this.pointer = 1; }
        exec(sql) {
          sql = typeof sql === "string" ? sql : sql.sql;
          const pragma = /^PRAGMA\s+(\w+)(?:\s*=\s*(\S+))?/i.exec(sql);
          if (pragma && authorizer?.(0, 19, pragma[1], pragma[2] ?? null, "main", null) === 1) {
            const error = new Error("not authorized");
            error.resultCode = 23;
            throw error;
          }
          worker.commands.push(sql.trim().split(/\s+/).slice(0, 2).join(" "));
        }
        changes() { return 0; }
      }
      const sqlite = {
        oo1: { OpfsWlDb: Database },
        capi: {
          SQLITE_OK: 0, SQLITE_DENY: 1, SQLITE_UTF8: 1,
          sqlite3_set_authorizer(_pointer, callback) { authorizer = callback; return 0; },
          sqlite3_create_function_v2() {},
          sqlite3_last_insert_rowid() { return 0; },
        },
      };
      const context = vm.createContext({
        console, URL, TextEncoder, crypto: webcrypto, setTimeout, self: endpoint,
        location: { href: String(url) }, navigator: { locks: locks.forContext(owner), storage: opfs.storage },
      });
      const dependency = new vm.SyntheticModule(["default"], function() {
        this.setExport("default", async () => sqlite);
      }, { context });
      const module = new vm.SourceTextModule(workerSource, moduleOptions(context, String(url)));
      module.link(() => dependency).then(() => module.evaluate()).catch((error) => {
        this.listeners.get("error")?.forEach((callback) => callback({ message: error.message }));
      });
    }
    addEventListener(type, callback) {
      if (!this.listeners.has(type)) this.listeners.set(type, []);
      this.listeners.get(type).push(callback);
    }
    postMessage(data) {
      queueMicrotask(() => {
        if (this.terminated) return;
        if (data?.type === "client-closed") options.onWorkerClientClosed?.(data.clientId);
        this.onMessage({ data });
      });
    }
    terminate() { this.terminated = true; }
  }

  async function page(name) {
    const context = vm.createContext({
      console, URL, TextEncoder, crypto: webcrypto, AbortController, setTimeout,
      navigator: { locks: {
        request(lock, ...arguments_) {
          if (options.denyLifetimeLock && lock.includes(":client:")) {
            return Promise.reject(new Error("client lock denied"));
          }
          return locks.forContext(name).request(lock, ...arguments_);
        },
      } }, location: { pathname: "/test" },
      Worker: class extends Worker { constructor(url) { super(url, name); } },
      BroadcastChannel: class extends Channel { constructor(channel) { super(channel, name); } },
    });
    const module = new vm.SourceTextModule(bridgeSource, moduleOptions(context, "https://example.test/fission-sqlite.mjs"));
    await module.link(() => { throw new Error("unexpected import"); });
    await module.evaluate();
    module.namespace.installFissionSqlite({ appId: "sqlite-bridge-lifetime-test" });
    const page = {
      name,
      request: (operation, request = {}) => context.__fissionSqliteRequest({ databaseName: "workspace", operation, request }),
      close: () => context.__fissionSqliteCloseDatabase("workspace"),
      shutdown: () => context.__FISSION_SQLITE_BRIDGE__.close(),
      destroy() {
        for (const peers of channels.values()) for (const channel of peers) {
          if (channel.owner === name) channel.close();
        }
        for (const worker of workers) if (worker.owner === name) worker.terminate();
        locks.destroy(name);
      },
    };
    pages.push(page);
    return page;
  }

  return { page, workers, opfs, close() { for (const page of pages) page.shutdown(); } };
}

export async function deadline(promise, ms = 2000) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`test operation timed out after ${ms}ms`)), ms);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

export async function beginWhenAvailable(page) {
  const until = Date.now() + 2000;
  while (Date.now() < until) {
    try { return await deadline(page.request("sql_tx_begin"), Math.max(1, until - Date.now())); }
    catch (error) { if (error.code !== "busy") throw error; }
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  throw new Error("database remained busy after its client disconnected");
}
