const applicationId = new URL(globalThis.location.href).searchParams.get("fission_app_id");
if (!applicationId) {
  throw new Error("Fission SQLite broker started without an application ID.");
}

const databaseWorkerUrl = new URL("./fission-sqlite-worker.mjs", import.meta.url);
databaseWorkerUrl.searchParams.set("fission_app_id", applicationId);
const databaseWorker = new Worker(databaseWorkerUrl, {
  type: "module",
  name: `fission-sqlite-database:${applicationId}`,
});

let nextRelayId = 1;
const pending = new Map();

databaseWorker.addEventListener("message", ({ data }) => {
  if (data?.type === "ready") return;
  const request = pending.get(data?.id);
  if (!request) return;
  pending.delete(data.id);
  request.port.postMessage({ ...data, id: request.clientId });
});

databaseWorker.addEventListener("error", (event) => {
  const message = event.message || "Fission SQLite database worker failed";
  for (const request of pending.values()) {
    request.port.postMessage({ id: request.clientId, ok: false, error: message });
  }
  pending.clear();
});

self.addEventListener("connect", ({ ports }) => {
  const port = ports[0];
  port.addEventListener("message", ({ data }) => {
    const relayId = nextRelayId++;
    pending.set(relayId, { port, clientId: data.id });
    databaseWorker.postMessage({ ...data, id: relayId });
  });
  port.start();
  port.postMessage({ type: "ready", applicationId });
});
