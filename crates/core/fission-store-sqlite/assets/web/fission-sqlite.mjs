const worker = new Worker(new URL("./fission-sqlite-worker.mjs", import.meta.url), {
  type: "module",
  name: "fission-sqlite",
});

let nextId = 1;
const pending = new Map();

worker.addEventListener("message", ({ data }) => {
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

export function installFissionSqlite() {
  globalThis.__fissionSqliteRequest = (request) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      worker.postMessage({ id, request });
    });
}
