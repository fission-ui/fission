import sqlite3InitModule from "./sqlite3.mjs";

const applicationId = new URL(globalThis.location.href).searchParams.get("fission_app_id");
if (!applicationId) {
  throw new Error("Fission SQLite worker started without an application ID.");
}

const namespaceDigest = new Uint8Array(
  await crypto.subtle.digest("SHA-256", new TextEncoder().encode(applicationId)),
);
const namespace = Array.from(namespaceDigest.slice(0, 16), (byte) =>
  byte.toString(16).padStart(2, "0")
).join("");
const databaseFilename = `/fission-apps/${namespace}/store.sqlite3`;

const sqlite3 = await sqlite3InitModule({
  locateFile: (file) => new URL(file, import.meta.url).href,
});

if (!sqlite3.oo1.OpfsWlDb) {
  throw new Error(
    "Fission Web SQLite requires the opfs-wl VFS. Serve the application with " +
    "Cross-Origin-Opener-Policy: same-origin and " +
    "Cross-Origin-Embedder-Policy: require-corp, and use a browser with OPFS, " +
    "Web Locks, SharedArrayBuffer, and Atomics.waitAsync support.",
  );
}

async function legacyPoolExists() {
  try {
    const root = await navigator.storage.getDirectory();
    await root.getDirectoryHandle("fission");
    return true;
  } catch (_) {
    return false;
  }
}

async function opfsFileExists(filename) {
  try {
    const parts = filename.split("/").filter(Boolean);
    const basename = parts.pop();
    let directory = await navigator.storage.getDirectory();
    for (const part of parts) {
      directory = await directory.getDirectoryHandle(part);
    }
    await directory.getFileHandle(basename);
    return true;
  } catch (_) {
    return false;
  }
}

async function migrateLegacyDatabase() {
  // sqlite3.opfs is an initialization-only namespace that official release
  // builds delete before sqlite3InitModule() resolves. Query OPFS directly.
  if (await opfsFileExists(databaseFilename)) return;
  if (!(await legacyPoolExists())) return;

  await navigator.locks.request(`fission-sqlite-migration:${namespace}`, async () => {
    if (await opfsFileExists(databaseFilename)) return;

    let pool;
    try {
      pool = await sqlite3.installOpfsSAHPoolVfs({
        name: `fission-opfs-migration-${namespace}`,
        directory: "fission",
      });
      const legacyName = pool
        .getFileNames()
        .find((name) => name === "/store.sqlite3" || name === "store.sqlite3");
      if (!legacyName) return;
      const bytes = pool.exportFile(legacyName);
      pool.pauseVfs();
      await sqlite3.oo1.OpfsWlDb.importDb(databaseFilename, bytes);
    } catch (error) {
      throw new Error(
        "Fission could not migrate the existing Web SQLite database. Close pages " +
        "running an older Fission build and reload: " +
        (error instanceof Error ? error.message : String(error)),
      );
    } finally {
      if (pool && !pool.isPaused()) {
        try {
          pool.pauseVfs();
        } catch (_) {
          // The original error is more useful than a secondary cleanup error.
        }
      }
    }
  });
}

await migrateLegacyDatabase();
const db = new sqlite3.oo1.OpfsWlDb(databaseFilename, "c");

db.exec(`
CREATE TABLE IF NOT EXISTS fission (
  scope TEXT NOT NULL,
  owner TEXT NOT NULL DEFAULT '',
  namespace TEXT NOT NULL,
  key TEXT NOT NULL,
  value BLOB NOT NULL,
  revision INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (scope, owner, namespace, key)
) WITHOUT ROWID;
`);

function sqlValue(value) {
  if (value === "Null" || value == null) return null;
  if (Object.hasOwn(value, "Integer")) return value.Integer;
  if (Object.hasOwn(value, "Real")) return value.Real;
  if (Object.hasOwn(value, "Text")) return value.Text;
  if (Object.hasOwn(value, "Blob")) return new Uint8Array(value.Blob);
  throw new Error("invalid SQL value");
}

function wireValue(value, sqliteType) {
  switch (sqliteType) {
    case sqlite3.capi.SQLITE_NULL: return "Null";
    case sqlite3.capi.SQLITE_INTEGER: return { Integer: value };
    case sqlite3.capi.SQLITE_FLOAT: return { Real: value };
    case sqlite3.capi.SQLITE_BLOB: return { Blob: Array.from(value) };
    case sqlite3.capi.SQLITE_TEXT: return { Text: value };
    default: throw new Error(`unsupported SQLite value type: ${sqliteType}`);
  }
}

function bindings(parameters) {
  if (!parameters || parameters === "None") return undefined;
  if (parameters.Positional) return parameters.Positional.map(sqlValue);
  if (parameters.Named) {
    return Object.fromEntries(parameters.Named.map(([name, value]) => [name, sqlValue(value)]));
  }
  throw new Error("invalid SQL parameters");
}

function execute(statement) {
  const before = db.changes(true, true);
  db.exec({ sql: statement.sql, bind: bindings(statement.parameters) });
  return {
    affected_rows: db.changes(true, true) - before,
    last_insert_rowid: db.selectValue("SELECT last_insert_rowid()"),
  };
}

function query(statement) {
  const prepared = db.prepare(statement.sql);
  try {
    const bound = bindings(statement.parameters);
    if (bound !== undefined) prepared.bind(bound);
    const columns = prepared
      .getColumnNames()
      .map((name) => ({ name, declared_type: null }));
    const rows = [];
    while (prepared.step()) {
      const values = prepared.get([]).map((value, index) =>
        wireValue(value, sqlite3.capi.sqlite3_column_type(prepared.pointer, index))
      );
      rows.push({ columns, values });
    }
    return { columns, rows };
  } finally {
    prepared.finalize();
  }
}

function scopeParts(scope) {
  if (scope === "Application") return ["application", ""];
  if (scope.Session !== undefined) return ["session", scope.Session];
  if (scope.User !== undefined) return ["user", scope.User];
  if (scope.Named !== undefined) return [scope.Named.scope, scope.Named.owner];
  throw new Error("invalid store scope");
}

function addressBindings(address) {
  const [scope, owner] = scopeParts(address.scope);
  return [scope, owner, address.namespace, address.key];
}

function storeGet(request) {
  const row = db.selectArray(
    "SELECT value FROM fission WHERE scope=?1 AND owner=?2 AND namespace=?3 AND key=?4",
    addressBindings(request.address),
  );
  return row ? Array.from(row[0]) : null;
}

function storeContains(request) {
  return Boolean(db.selectValue(
    "SELECT EXISTS(SELECT 1 FROM fission WHERE scope=?1 AND owner=?2 AND namespace=?3 AND key=?4)",
    addressBindings(request.address),
  ));
}

function storeSet(request) {
  db.exec({
    sql: `INSERT INTO fission(scope,owner,namespace,key,value,revision)
          VALUES(?1,?2,?3,?4,?5,1)
          ON CONFLICT(scope,owner,namespace,key) DO UPDATE SET
          value=excluded.value, revision=fission.revision+1`,
    bind: [...addressBindings(request.address), new Uint8Array(request.value)],
  });
}

function storeRemove(request) {
  const before = db.changes(true, true);
  db.exec({
    sql: "DELETE FROM fission WHERE scope=?1 AND owner=?2 AND namespace=?3 AND key=?4",
    bind: addressBindings(request.address),
  });
  return db.changes(true, true) > before;
}

function storeBatch(request) {
  let sets = 0;
  let removals = 0;
  db.transaction(() => {
    for (const operation of request.operations) {
      if (operation.Set) {
        storeSet(operation.Set);
        sets += 1;
      } else if (operation.Remove && storeRemove(operation.Remove)) {
        removals += 1;
      }
    }
  });
  return { sets, removals };
}

function storeListPrefix(request) {
  const [scope, owner] = scopeParts(request.scope);
  const rows = db.selectArrays(
    `SELECT key,value,revision FROM fission
     WHERE scope=?1 AND owner=?2 AND namespace=?3
       AND substr(key,1,length(?4))=?4 ORDER BY key`,
    [scope, owner, request.namespace, request.prefix],
  );
  return rows.map(([key, value, revision]) => ({
    address: { scope: request.scope, namespace: request.namespace, key },
    value: Array.from(value),
    revision,
  }));
}

function sqlTransaction(request) {
  const steps = [];
  db.transaction(() => {
    for (const step of request.steps) {
      if (step.Execute) steps.push({ Execute: execute(step.Execute) });
      else steps.push({ Query: query(step.Query.statement) });
    }
  });
  return { steps };
}

function sqlMigrate(request) {
  const previousVersion = Number(db.selectValue("PRAGMA user_version"));
  let currentVersion = previousVersion;
  let applied = 0;
  const migrations = Object.values(request.migrations)
    .filter((migration) => migration.version > previousVersion)
    .sort((left, right) => left.version - right.version);
  db.transaction(() => {
    for (const migration of migrations) {
      db.exec(migration.sql);
      db.exec(`PRAGMA user_version = ${Number(migration.version)}`);
      currentVersion = migration.version;
      applied += 1;
    }
  });
  return {
    previous_version: previousVersion,
    current_version: currentVersion,
    applied,
  };
}

function dispatch(operation, request) {
  switch (operation) {
    case "store_get": return storeGet(request);
    case "store_contains": return storeContains(request);
    case "store_set": storeSet(request); return null;
    case "store_remove": return storeRemove(request);
    case "store_batch": return storeBatch(request);
    case "store_list_prefix": return storeListPrefix(request);
    case "sql_execute": return execute(request);
    case "sql_query": return query(request.statement);
    case "sql_transaction": return sqlTransaction(request);
    case "sql_migrate": return sqlMigrate(request);
    default: throw new Error(`unsupported SQLite operation: ${operation}`);
  }
}

function handleMessage(endpoint, data) {
  try {
    const { operation, request } = data.request;
    endpoint.postMessage({ id: data.id, ok: true, value: dispatch(operation, request) });
  } catch (error) {
    endpoint.postMessage({
      id: data.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
}

function connect(endpoint) {
  endpoint.addEventListener("message", ({ data }) => handleMessage(endpoint, data));
  endpoint.start?.();
  endpoint.postMessage({ type: "ready", applicationId });
}

if (typeof SharedWorkerGlobalScope !== "undefined") {
  self.addEventListener("connect", ({ ports }) => connect(ports[0]));
} else {
  connect(self);
}
