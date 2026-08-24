import sqlite3InitModule from "./sqlite3.mjs";

const sqlite3 = await sqlite3InitModule({
  locateFile: (file) => new URL(file, import.meta.url).href,
});
await sqlite3.installOpfsSAHPoolVfs({
  name: "fission-opfs",
  directory: "fission",
});
const db = new sqlite3.oo1.DB("file:/store.sqlite3?vfs=fission-opfs", "c");

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

function wireValue(value) {
  if (value == null) return "Null";
  if (value instanceof Uint8Array) return { Blob: Array.from(value) };
  if (typeof value === "bigint") return { Integer: value };
  if (typeof value === "number") {
    return Number.isInteger(value) ? { Integer: value } : { Real: value };
  }
  return { Text: String(value) };
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
  const resultRows = [];
  const columnNames = [];
  db.exec({
    sql: statement.sql,
    bind: bindings(statement.parameters),
    rowMode: "array",
    resultRows,
    columnNames,
  });
  const columns = columnNames.map((name) => ({ name, declared_type: null }));
  return {
    columns,
    rows: resultRows.map((values) => ({
      columns,
      values: values.map(wireValue),
    })),
  };
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

self.addEventListener("message", ({ data }) => {
  try {
    const { operation, request } = data.request;
    self.postMessage({ id: data.id, ok: true, value: dispatch(operation, request) });
  } catch (error) {
    self.postMessage({
      id: data.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
});

self.postMessage({ type: "ready" });
