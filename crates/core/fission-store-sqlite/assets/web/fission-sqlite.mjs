const BRIDGE_KEY = "__FISSION_SQLITE_BRIDGE__";

function defaultApplicationId() {
  const path = globalThis.location?.pathname
    ?.split("/")
    .filter(Boolean)[0];
  return path ? `path:${path}` : "fission-web-app";
}

function createDatabaseBridge(applicationId, databaseName, databaseId) {
  const workerUrl = new URL("./fission-sqlite-worker.mjs", import.meta.url);
  workerUrl.searchParams.set("fission_app_id", applicationId);
  if (databaseId) workerUrl.searchParams.set("fission_database_id", databaseId);
  const contextId = crypto.randomUUID();
  const databaseSuffix = databaseId ? `:db:${databaseId}` : "";
  const channelName = `fission-sqlite:${applicationId}${databaseSuffix}`;
  const lockName = `fission-sqlite-owner:${applicationId}${databaseSuffix}`;
  const clientLockName = (id) => `${lockName}:client:${id}`;
  const coordinated =
    typeof BroadcastChannel === "function" && navigator.locks?.request;

  let nextClientId = 1;
  let nextRelayId = 1;
  let worker = null;
  let workerReady = false;
  let ownerId = null;
  let releaseOwnership = null;
  let releaseClientLifetime = null;
  let clientReady = !coordinated;
  let closed = false;
  let terminalError = null;
  let clientLifetimeError = null;
  let workerRestartCount = 0;
  const pending = new Map();
  const relays = new Map();
  const waitingEnvelopes = [];
  const channel = coordinated ? new BroadcastChannel(channelName) : null;
  const ownershipAbort = coordinated ? new AbortController() : null;
  const clientLifetimeAbort = coordinated ? new AbortController() : null;
  const clientWatchers = new Map();

  function failPendingRequests(message) {
    terminalError = new Error(message);
    terminalError.code = "unavailable";
    for (const request of pending.values()) request.reject(terminalError);
    pending.clear();
  }

  function failStartup(message) {
    failPendingRequests(message);
    for (const envelope of waitingEnvelopes.splice(0)) {
      respond(envelope.sender, {
        id: envelope.requestId, ok: false, code: "unavailable", message,
      });
    }
    channel?.postMessage({ type: "owner-unavailable", ownerId: contextId, message });
  }

  function disconnectClient(clientId) {
    const watcher = clientWatchers.get(clientId);
    clientWatchers.delete(clientId);
    watcher?.abort();
    // Discard requests not yet sent to the worker. Requests already forwarded
    // precede this message on the same worker port, including an in-flight BEGIN.
    for (let index = waitingEnvelopes.length - 1; index >= 0; index -= 1) {
      if (waitingEnvelopes[index].sender === clientId) waitingEnvelopes.splice(index, 1);
    }
    worker?.postMessage({ type: "client-closed", clientId });
  }

  function watchClient(clientId) {
    if (!coordinated || clientId === contextId || clientWatchers.has(clientId)) return;
    const abort = new AbortController();
    clientWatchers.set(clientId, abort);
    // Every sender holds this exclusive lifetime lock before sending requests.
    // A shared waiter runs when the browser releases it on close/navigation or
    // context destruction. Unlike a heartbeat lease, background throttling does
    // not expire a live transaction. This watches transport lifetime only; the
    // SQLite worker remains the sole transaction authority.
    navigator.locks.request(
      clientLockName(clientId),
      { mode: "shared", signal: abort.signal },
      () => {
        if (clientWatchers.get(clientId) === abort) disconnectClient(clientId);
      },
    ).catch((error) => {
      if (error?.name !== "AbortError") {
        // Losing the lifetime monitor cannot justify keeping a borrowed
        // transaction open indefinitely.
        disconnectClient(clientId);
      }
    });
  }

  function settle(data) {
    const request = pending.get(data.id);
    if (!request) return;
    pending.delete(data.id);
    if (data.ok) request.resolve(data.value);
    else {
      const error = new Error(data.message || data.error || "SQLite worker request failed");
      if (data.code) error.code = data.code;
      if (Number.isInteger(data.sqlite_code)) error.sqliteCode = data.sqlite_code;
      request.reject(error);
    }
  }

  function respond(recipient, data) {
    if (recipient === contextId) settle(data);
    else channel.postMessage({ type: "response", recipient, data });
  }

  function forward(envelope) {
    if (!workerReady || envelope.ownerId !== ownerId) return;
    watchClient(envelope.sender);
    const relayId = nextRelayId++;
    relays.set(relayId, {
      sender: envelope.sender,
      requestId: envelope.requestId,
      transactional: envelope.request?.operation?.startsWith("sql_tx_") || false,
    });
    worker.postMessage({
      id: relayId,
      clientId: envelope.sender,
      request: envelope.request,
    });
  }

  function sendPendingToOwner() {
    if (!ownerId || !clientReady || closed) return;
    for (const request of pending.values()) {
      if (request.ownerId === ownerId) continue;
      const envelope = {
        type: "request",
        ownerId,
        sender: contextId,
        requestId: request.id,
        request: request.value,
      };
      if (ownerId === contextId) {
        if (!workerReady) continue;
        request.ownerId = ownerId;
        forward(envelope);
      } else if (channel) {
        request.ownerId = ownerId;
        channel.postMessage(envelope);
      }
    }
  }

  function failDatabaseWorker(currentWorker, reason) {
    if (worker !== currentWorker) return;
    workerReady = false;
    worker = null;
    currentWorker.terminate();
    for (const [relayId, relay] of relays) {
      relays.delete(relayId);
      respond(relay.sender, {
        id: relay.requestId,
        ok: false,
        code: relay.transactional ? "transaction_lost" : "unavailable",
        message: `SQLite worker stopped before replying: ${reason}`,
      });
    }
    if (!closed && workerRestartCount < 3) {
      workerRestartCount += 1;
      setTimeout(() => {
        if (!closed && !worker && ownerId === contextId) startDatabaseWorker();
      }, workerRestartCount * 100);
    } else if (!closed) {
      failStartup(`SQLite worker could not start after repeated failures: ${reason}. Close and reopen the database to retry.`);
    }
  }

  function startDatabaseWorker() {
    if (closed || worker) return;
    ownerId = contextId;
    let currentWorker;
    try {
      currentWorker = new Worker(workerUrl, {
        type: "module",
        name: `fission-sqlite:${applicationId}${databaseSuffix}`,
      });
    } catch (error) {
      failStartup(`SQLite worker could not be created: ${error?.message || error}`);
      return;
    }
    worker = currentWorker;

    currentWorker.addEventListener("message", ({ data }) => {
      if (worker !== currentWorker) return;
      if (data?.type === "ready") {
        workerReady = true;
        terminalError = null;
        workerRestartCount = 0;
        channel?.postMessage({ type: "owner-ready", ownerId });
        for (const envelope of waitingEnvelopes.splice(0)) forward(envelope);
        sendPendingToOwner();
        return;
      }
      const relay = relays.get(data?.id);
      if (!relay) return;
      relays.delete(data.id);
      respond(relay.sender, { ...data, id: relay.requestId });
    });

    currentWorker.addEventListener("error", (event) => {
      failDatabaseWorker(currentWorker, event.message || "SQLite worker failed");
    });
    currentWorker.addEventListener("messageerror", () => {
      failDatabaseWorker(currentWorker, "the worker returned an unreadable message");
    });
  }

  if (coordinated) {
    channel.addEventListener("message", ({ data }) => {
      if (data?.type === "discover-owner") {
        if (workerReady) channel.postMessage({ type: "owner-ready", ownerId });
        else if (ownerId === contextId && terminalError) {
          channel.postMessage({ type: "owner-unavailable", ownerId, message: terminalError.message });
        }
      } else if (data?.type === "owner-ready") {
        ownerId = data.ownerId;
        terminalError = null;
        sendPendingToOwner();
      } else if (data?.type === "owner-unavailable") {
        if (!ownerId || ownerId === data.ownerId) {
          ownerId = data.ownerId;
          failPendingRequests(data.message || "SQLite owner worker is unavailable");
        }
      } else if (data?.type === "request") {
        if (data.ownerId === ownerId) {
          if (workerReady) forward(data);
          else waitingEnvelopes.push(data);
        }
      } else if (data?.type === "client-closed") {
        if (ownerId === contextId && data.ownerId === contextId) {
          disconnectClient(data.sender);
        }
      } else if (data?.type === "response" && data.recipient === contextId) {
        settle(data.data);
      }
    });

    navigator.locks.request(
      clientLockName(contextId),
      { mode: "exclusive", signal: clientLifetimeAbort.signal },
      async () => {
        if (closed) return;
        // Set the release callback before publishing readiness; a synchronous
        // close must never leave the lifetime lock held.
        const lifetime = new Promise((resolve) => { releaseClientLifetime = resolve; });
        clientReady = true;
        sendPendingToOwner();
        await lifetime;
      },
    ).catch((error) => {
      if (error?.name !== "AbortError") {
        failPendingRequests(`SQLite client lifetime lock could not be acquired: ${error?.message || error}`);
        clientLifetimeError = terminalError;
      }
    });

    navigator.locks
      .request(lockName, { mode: "exclusive", signal: ownershipAbort.signal }, async () => {
        if (closed) return;
        startDatabaseWorker();
        await new Promise((resolve) => {
          releaseOwnership = resolve;
        });
      })
      .catch((error) => {
        if (error?.name !== "AbortError") {
          failStartup(`SQLite owner lock could not be acquired: ${error?.message || error}`);
        }
      });
    channel.postMessage({ type: "discover-owner" });
  } else {
    startDatabaseWorker();
  }

  const request = (value) =>
    new Promise((resolve, reject) => {
      if (clientLifetimeError || terminalError) {
        reject(clientLifetimeError || terminalError);
        return;
      }
      if (closed) {
        reject(new Error("SQLite bridge closed"));
        return;
      }
      const id = nextClientId++;
      pending.set(id, { id, value, resolve, reject, ownerId: null });
      if (ownerId) sendPendingToOwner();
      else channel?.postMessage({ type: "discover-owner" });
    });

  const close = () => {
    if (closed) return;
    closed = true;
    if (coordinated && ownerId && ownerId !== contextId) {
      channel.postMessage({ type: "client-closed", ownerId, sender: contextId });
    }
    for (const request of pending.values()) {
      request.reject(new Error("SQLite bridge closed"));
    }
    pending.clear();
    clientLifetimeAbort?.abort();
    releaseClientLifetime?.();
    for (const watcher of clientWatchers.values()) watcher.abort();
    clientWatchers.clear();
    ownershipAbort?.abort();
    worker?.terminate();
    releaseOwnership?.();
    channel?.close();
  };

  return { applicationId, databaseName, request, close };
}

async function databaseDigest(name) {
  const digest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(name)),
  );
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function createBridge(applicationId) {
  const databases = new Map();
  let closed = false;

  async function request(value) {
    if (closed) throw new Error("SQLite bridge closed");
    if (!value || typeof value !== "object") {
      const error = new Error("invalid SQLite database request");
      error.code = "invalid_request";
      throw error;
    }

    const name = value.databaseName;
    if (
      name !== undefined &&
      (typeof name !== "string" || name.trim().length === 0 ||
        new TextEncoder().encode(name).length > 1024)
    ) {
      const error = new Error("Web SQLite database names must contain 1 to 1024 UTF-8 bytes");
      error.code = "invalid_request";
      throw error;
    }

    const key = name === undefined ? null : name;
    let database = databases.get(key);
    if (!database) {
      const id = name === undefined ? null : await databaseDigest(name);
      if (closed) throw new Error("SQLite bridge closed");
      database = databases.get(key);
      if (!database) {
        database = createDatabaseBridge(applicationId, name, id);
        databases.set(key, database);
      }
    }

    const { databaseName: _databaseName, ...databaseRequest } = value;
    return database.request(databaseRequest);
  }

  function closeDatabase(name) {
    const key = name === undefined ? null : name;
    const database = databases.get(key);
    if (!database) return false;
    databases.delete(key);
    database.close();
    return true;
  }

  function close() {
    closed = true;
    for (const database of databases.values()) database.close();
    databases.clear();
  }

  return { applicationId, request, close, closeDatabase };
}

/**
 * Installs the browser bridge used by `WebSqliteStore`.
 *
 * Repeated installation for the same application is idempotent. Supplying a
 * different application ID in one page is rejected because a Fission page has
 * exactly one application storage namespace.
 */
export function installFissionSqlite(options = {}) {
  const applicationId = options.appId || defaultApplicationId();
  const installed = globalThis[BRIDGE_KEY];
  if (installed && installed.applicationId !== applicationId) {
    throw new Error(
      `Fission SQLite is already installed for ${installed.applicationId}; ` +
      `cannot reinstall it for ${applicationId}.`,
    );
  }

  const bridge = installed || createBridge(applicationId);
  if (!installed) {
    Object.defineProperty(globalThis, BRIDGE_KEY, {
      configurable: false,
      enumerable: false,
      writable: false,
      value: bridge,
    });
  }
  globalThis.__fissionSqliteRequest = bridge.request;
  globalThis.__fissionSqliteCloseDatabase = bridge.closeDatabase;
}
