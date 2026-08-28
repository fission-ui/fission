const BRIDGE_KEY = "__FISSION_SQLITE_BRIDGE__";

function defaultApplicationId() {
  const path = globalThis.location?.pathname
    ?.split("/")
    .filter(Boolean)[0];
  return path ? `path:${path}` : "fission-web-app";
}

function createBridge(applicationId) {
  const workerName = `fission-sqlite:${applicationId}`;
  const shared = typeof SharedWorker === "function";
  const workerUrl = new URL(
    shared ? "./fission-sqlite-broker.mjs" : "./fission-sqlite-worker.mjs",
    import.meta.url,
  );
  workerUrl.searchParams.set("fission_app_id", applicationId);
  const worker = shared
    ? new SharedWorker(workerUrl, { type: "module", name: workerName })
    : new Worker(workerUrl, { type: "module", name: workerName });
  const endpoint = shared ? worker.port : worker;
  endpoint.start?.();

  let nextId = 1;
  const pending = new Map();

  endpoint.addEventListener("message", ({ data }) => {
    if (data?.type === "ready") return;
    const request = pending.get(data?.id);
    if (!request) return;
    pending.delete(data.id);
    if (data.ok) request.resolve(data.value);
    else request.reject(new Error(data.error || "SQLite worker request failed"));
  });

  worker.addEventListener("error", (event) => {
    const error = event.error || new Error(event.message || "SQLite worker failed");
    for (const request of pending.values()) request.reject(error);
    pending.clear();
  });

  const request = (value) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      endpoint.postMessage({ id, request: value });
    });

  const close = () => {
    for (const request of pending.values()) {
      request.reject(new Error("SQLite bridge closed"));
    }
    pending.clear();
    if (shared) endpoint.close();
    else worker.terminate();
  };

  return { applicationId, request, worker, close };
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
