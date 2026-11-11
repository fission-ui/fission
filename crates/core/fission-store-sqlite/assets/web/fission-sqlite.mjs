const BRIDGE_KEY = "__FISSION_SQLITE_BRIDGE__";

function defaultApplicationId() {
  const path = globalThis.location?.pathname
    ?.split("/")
    .filter(Boolean)[0];
  return path ? `path:${path}` : "fission-web-app";
}

function createBridge(applicationId) {
  const workerUrl = new URL("./fission-sqlite-worker.mjs", import.meta.url);
  workerUrl.searchParams.set("fission_app_id", applicationId);
  const contextId = crypto.randomUUID();
  const channelName = `fission-sqlite:${applicationId}`;
  const lockName = `fission-sqlite-owner:${applicationId}`;
  const coordinated =
    typeof BroadcastChannel === "function" && navigator.locks?.request;

  let nextClientId = 1;
  let nextRelayId = 1;
  let worker = null;
  let workerReady = false;
  let ownerId = null;
  let releaseOwnership = null;
  let closed = false;
  let workerRestartCount = 0;
  const pending = new Map();
  const relays = new Map();
  const waitingEnvelopes = [];
  const channel = coordinated ? new BroadcastChannel(channelName) : null;
  const ownershipAbort = coordinated ? new AbortController() : null;

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
    if (!ownerId) return;
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
    }
  }

  function startDatabaseWorker() {
    if (closed || worker) return;
    ownerId = contextId;
    const currentWorker = new Worker(workerUrl, {
      type: "module",
      name: `fission-sqlite:${applicationId}`,
    });
    worker = currentWorker;

    currentWorker.addEventListener("message", ({ data }) => {
      if (worker !== currentWorker) return;
      if (data?.type === "ready") {
        workerReady = true;
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
      } else if (data?.type === "owner-ready") {
        ownerId = data.ownerId;
        sendPendingToOwner();
      } else if (data?.type === "request") {
        if (data.ownerId === ownerId) {
          if (workerReady) forward(data);
          else waitingEnvelopes.push(data);
        }
      } else if (data?.type === "response" && data.recipient === contextId) {
        settle(data.data);
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
        if (error?.name !== "AbortError") console.error(error);
      });
    channel.postMessage({ type: "discover-owner" });
  } else {
    startDatabaseWorker();
  }

  const request = (value) =>
    new Promise((resolve, reject) => {
      const id = nextClientId++;
      pending.set(id, { id, value, resolve, reject, ownerId: null });
      if (ownerId) sendPendingToOwner();
      else channel?.postMessage({ type: "discover-owner" });
    });

  const close = () => {
    closed = true;
    for (const request of pending.values()) {
      request.reject(new Error("SQLite bridge closed"));
    }
    pending.clear();
    ownershipAbort?.abort();
    worker?.terminate();
    releaseOwnership?.();
    channel?.close();
  };

  return { applicationId, request, close };
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
}
