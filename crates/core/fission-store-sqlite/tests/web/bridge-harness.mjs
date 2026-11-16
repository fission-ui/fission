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
        console, URL, TextEncoder, crypto: webcrypto, self: endpoint,
        location: { href: String(url) }, navigator: { locks: locks.forContext(owner) },
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
    postMessage(data) { queueMicrotask(() => { if (!this.terminated) this.onMessage({ data }); }); }
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

  return { page, workers, close() { for (const page of pages) page.shutdown(); } };
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
