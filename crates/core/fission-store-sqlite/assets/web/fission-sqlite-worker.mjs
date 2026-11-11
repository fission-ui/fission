import sqlite3InitModule from "./sqlite3.mjs";

const applicationId = new URL(globalThis.location.href).searchParams.get("fission_app_id");
const databaseId = new URL(globalThis.location.href).searchParams.get("fission_database_id");
if (!applicationId) {
  throw new Error("Fission SQLite worker started without an application ID.");
}
if (databaseId && !/^[0-9a-f]{64}$/.test(databaseId)) {
  throw new Error("Fission SQLite worker received an invalid database ID.");
}

const namespaceDigest = new Uint8Array(
  await crypto.subtle.digest("SHA-256", new TextEncoder().encode(applicationId)),
);
const namespace = Array.from(namespaceDigest.slice(0, 16), (byte) =>
  byte.toString(16).padStart(2, "0")
).join("");
const databaseFilename = databaseId
  ? `/fission-apps/${namespace}/store-${databaseId}.sqlite3`
  : `/fission-apps/${namespace}/store.sqlite3`;

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

// The legacy store.sqlite3 belongs to the application's default database.
// Named workspace databases always start independently and must never import
// the default database's tables or data.
if (!databaseId) await migrateLegacyDatabase();
const db = new sqlite3.oo1.OpfsWlDb(databaseFilename, "c");
let activeTransaction = null;
let activeAuthorizerPolicy = null;
let allowManagedTransactionControl = false;

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

function withManagedTransaction(operation) {
  allowManagedTransactionControl = true;
  try {
    db.exec("BEGIN IMMEDIATE");
  } finally {
    allowManagedTransactionControl = false;
  }
  try {
    const result = operation();
    allowManagedTransactionControl = true;
    try {
      db.exec("COMMIT");
    } finally {
      allowManagedTransactionControl = false;
    }
    return result;
  } catch (error) {
    try {
      allowManagedTransactionControl = true;
      db.exec("ROLLBACK");
    } catch (_) {
      // SQLite may already have rolled back the transaction after a statement error.
    } finally {
      allowManagedTransactionControl = false;
    }
    throw error;
  }
}

function execute(statement, alreadyInTransaction = false) {
  const before = db.changes(true, true);
  const run = () => db.exec({ sql: statement.sql, bind: bindings(statement.parameters) });
  if (alreadyInTransaction) run();
  else withManagedTransaction(run);
  return {
    affected_rows: db.changes(true, true) - before,
    last_insert_rowid: sqlite3.capi.sqlite3_last_insert_rowid(db.pointer),
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
  withManagedTransaction(() => {
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
  withManagedTransaction(() => {
    for (const step of request.steps) {
      if (step.Execute) steps.push({ Execute: execute(step.Execute, true) });
      else steps.push({ Query: query(step.Query.statement) });
    }
  });
  return { steps };
}

function sqlMigrate(request) {
  const previousVersion = Number(db.selectValue("PRAGMA user_version"));
  let currentVersion = previousVersion;
  let applied = 0;
  // serde_wasm_bindgen serialises SqlMigrations' BTreeMap as a JS Map.
  const migrations = Array.from(request.migrations.values())
    .filter((migration) => migration.version > previousVersion)
    .sort((left, right) => left.version - right.version);
  withManagedTransaction(() => {
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

const authorizerActions = {
  CreateIndex: 1, CreateTable: 2, CreateTempIndex: 3, CreateTempTable: 4,
  CreateTempTrigger: 5, CreateTempView: 6, CreateTrigger: 7, CreateView: 8,
  Delete: 9, DropIndex: 10, DropTable: 11, DropTempIndex: 12,
  DropTempTable: 13, DropTempTrigger: 14, DropTempView: 15, DropTrigger: 16,
  DropView: 17, Insert: 18, Pragma: 19, Read: 20, Select: 21,
  Transaction: 22, Update: 23, Attach: 24, Detach: 25, AlterTable: 26,
  Reindex: 27, Analyze: 28, CreateVtable: 29, DropVtable: 30,
  Function: 31, Savepoint: 32, Recursive: 33,
};
const authorizerActionGroups = {
  Read: [19, 20, 21, 31],
  Write: [9, 18, 23],
  Attach: [24, 25],
  Admin: [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17, 26, 27, 28, 29, 30],
};

function sqliteFailure(code, message) {
  const error = new Error(message);
  error.resultCode = code;
  throw error;
}

function sqliteCheck(code, operation) {
  if (code !== sqlite3.capi.SQLITE_OK) {
    sqliteFailure(code, `SQLite ${operation} failed with result code ${code}`);
  }
}

function registerOperationContext(context) {
  if (!context) return;
  let nextSequence = Number(context.next_op_seq);
  if (!Number.isSafeInteger(nextSequence) || nextSequence < 0) {
    const error = new Error("operation context sequence must be a nonnegative safe integer");
    error.code = "invalid_request";
    throw error;
  }
  db.createFunction("pdb_current_operation_id", () => context.operation_id, { arity: 0 });
  db.createFunction("pdb_next_op_seq", () => nextSequence++, { arity: 0 });
}

function removeOperationContext() {
  const capi = sqlite3.capi;
  for (const name of ["pdb_current_operation_id", "pdb_next_op_seq"]) {
    capi.sqlite3_create_function_v2(db.pointer, name, 0, capi.SQLITE_UTF8, 0, 0, 0, 0, 0);
  }
}

function installAuthorizer(policy) {
  activeAuthorizerPolicy = policy || null;
}

function authorizerDecision(action, first, second, database, trigger) {
  const capi = sqlite3.capi;
  const policy = activeAuthorizerPolicy;
  if (!policy) return capi.SQLITE_OK;
  const rules = policy.rules || [];
  const defaultDecision = policy.defaultDecision === "Deny" ? "Deny" : "Allow";
  const matches = (expected, actual) => expected == null ||
    String(expected).toLocaleLowerCase() === String(actual ?? "").toLocaleLowerCase();
  const doesNotMatch = (unexpected, actual) => unexpected == null || !matches(unexpected, actual);
  const matchesPrefix = (expected, actual) => expected == null ||
    String(actual ?? "").toLocaleLowerCase().startsWith(String(expected).toLocaleLowerCase());
  const matchesSuffix = (expected, actual) => expected == null ||
    String(actual ?? "").toLocaleLowerCase().endsWith(String(expected).toLocaleLowerCase());
  const matchingRule = rules.find((rule) => {
    const actionCodes = rule.action === "Any"
      ? Array.from({ length: 33 }, (_, index) => index + 1)
      : authorizerActionGroups[rule.action]
      || [authorizerActions[rule.action]];
    return actionCodes.includes(action) &&
      matches(rule.firstArgument, first) &&
      matchesPrefix(rule.firstArgumentPrefix, first) &&
      matchesSuffix(rule.firstArgumentSuffix, first) &&
      matches(rule.secondArgument, second) &&
      matchesPrefix(rule.secondArgumentPrefix, second) &&
      matchesSuffix(rule.secondArgumentSuffix, second) &&
      matches(rule.database, database) &&
      doesNotMatch(rule.databaseNot, database) &&
      matches(rule.trigger, trigger);
  });
  const decision = matchingRule?.decision || defaultDecision;
  return decision === "Deny" ? capi.SQLITE_DENY : capi.SQLITE_OK;
}

function removeAuthorizer() {
  activeAuthorizerPolicy = null;
}

function installWorkerAuthorizer() {
  const capi = sqlite3.capi;
  const authorizer = (_context, action, first, second, database, trigger) => {
    if ((action === 22 || action === 32) && !allowManagedTransactionControl) {
      return capi.SQLITE_DENY;
    }
    return authorizerDecision(action, first, second, database, trigger);
  };
  sqliteCheck(capi.sqlite3_set_authorizer(db.pointer, authorizer, 0), "authorizer registration");
}

function deleteSessions(transaction) {
  for (const session of transaction.sessions.values()) {
    try {
      sqlite3.capi.sqlite3session_delete(session.pointer);
    } catch (_) {
      // The SQLite connection is closing or already rolled back.
    }
  }
  transaction.sessions.clear();
}

function clearTransaction(transaction, { rollback = false } = {}) {
  if (!transaction || activeTransaction !== transaction) return;
  removeAuthorizer();
  removeOperationContext();
  deleteSessions(transaction);
  if (rollback) {
    try {
      allowManagedTransactionControl = true;
      db.exec("ROLLBACK");
    } catch (_) {
      // SQLite may already have rolled the transaction back after an error.
    } finally {
      allowManagedTransactionControl = false;
    }
  }
  activeTransaction = null;
}

function transactionFor(clientId, request) {
  const transaction = activeTransaction;
  if (!transaction) {
    const error = new Error("The SQLite transaction is no longer active.");
    error.code = "transaction_lost";
    throw error;
  }
  if (transaction.clientId !== clientId) {
    const error = new Error("The SQLite transaction belongs to another browser context.");
    error.code = "transaction_owner_mismatch";
    throw error;
  }
  if (transaction.id !== request.transaction_id) {
    const error = new Error("The SQLite transaction handle is unknown or expired.");
    error.code = "transaction_lost";
    throw error;
  }
  return transaction;
}

function beginTransaction(clientId, request) {
  if (activeTransaction) {
    const error = new Error("Another browser context currently owns a SQLite transaction.");
    error.code = "busy";
    throw error;
  }

  allowManagedTransactionControl = true;
  try {
    db.exec("BEGIN IMMEDIATE");
  } finally {
    allowManagedTransactionControl = false;
  }
  const transaction = {
    id: crypto.randomUUID(),
    clientId,
    sessions: new Map(),
  };
  activeTransaction = transaction;
  try {
    registerOperationContext(request.operationContext);
    installAuthorizer(request.authorizerPolicy);
  } catch (error) {
    clearTransaction(transaction, { rollback: true });
    throw error;
  }
  return { transaction_id: transaction.id };
}

function createSession(transaction, request) {
  if (!sqlite3.capi.sqlite3session_create) {
    const error = new Error("This SQLite build does not include the Session extension.");
    error.code = "unavailable";
    throw error;
  }
  const wasm = sqlite3.wasm;
  const level = wasm.scopedAllocPush();
  let pointer = 0;
  try {
    const output = wasm.scopedAlloc(wasm.ptr.size);
    wasm.pokePtr(output, 0);
    sqliteCheck(sqlite3.capi.sqlite3session_create(db.pointer, "main", output), "session creation");
    pointer = wasm.peekPtr(output);
    for (const table of request.tables || []) {
      sqliteCheck(sqlite3.capi.sqlite3session_attach(pointer, table), `session attach for ${table}`);
    }
  } catch (error) {
    if (pointer) sqlite3.capi.sqlite3session_delete(pointer);
    throw error;
  } finally {
    wasm.scopedAllocPop(level);
  }

  const sessionId = crypto.randomUUID();
  transaction.sessions.set(sessionId, { pointer });
  return { session_id: sessionId };
}

function captureChangeset(transaction, request) {
  const session = transaction.sessions.get(request.session_id);
  if (!session) {
    const error = new Error("The SQLite session is unknown or expired.");
    error.code = "transaction_lost";
    throw error;
  }
  const wasm = sqlite3.wasm;
  const level = wasm.scopedAllocPush();
  let changesetPointer = 0;
  const authorizerPolicy = activeAuthorizerPolicy;
  try {
    const sizePointer = wasm.scopedAlloc(4);
    const outputPointer = wasm.scopedAlloc(wasm.ptr.size);
    wasm.poke(sizePointer, 0, "i32");
    wasm.pokePtr(outputPointer, 0);
    activeAuthorizerPolicy = null;
    allowManagedTransactionControl = true;
    try {
      sqliteCheck(
        sqlite3.capi.sqlite3session_changeset(session.pointer, sizePointer, outputPointer),
        "session changeset capture",
      );
    } finally {
      activeAuthorizerPolicy = authorizerPolicy;
      allowManagedTransactionControl = false;
    }
    const size = wasm.peek(sizePointer, "i32");
    changesetPointer = wasm.peekPtr(outputPointer);
    return size > 0
      ? Array.from(wasm.heap8u().slice(Number(changesetPointer), Number(changesetPointer) + size))
      : [];
  } finally {
    if (changesetPointer) sqlite3.capi.sqlite3_free(changesetPointer);
    wasm.scopedAllocPop(level);
  }
}

function applyChangeset(request) {
  const bytes = new Uint8Array(request.changeset || []);
  if (bytes.length === 0) return { applied: true };
  const wasm = sqlite3.wasm;
  const level = wasm.scopedAllocPush();
  try {
    const pointer = wasm.scopedAlloc(bytes.length);
    wasm.heap8u().set(bytes, Number(pointer));
    const decisions = { Omit: 0, Replace: 1, Abort: 2 };
    const conflict = decisions[request.conflict_policy || "Abort"];
    if (conflict === undefined) throw new Error("invalid changeset conflict policy");
    allowManagedTransactionControl = true;
    let result;
    try {
      result = sqlite3.capi.sqlite3changeset_apply(
        db.pointer,
        bytes.length,
        pointer,
        0,
        () => conflict,
        0,
      );
    } finally {
      allowManagedTransactionControl = false;
    }
    sqliteCheck(result, "changeset apply");
    return { applied: true };
  } finally {
    wasm.scopedAllocPop(level);
  }
}

function dispatchTransaction(operation, request, clientId) {
  if (operation === "sql_tx_begin") return beginTransaction(clientId, request);
  const transaction = transactionFor(clientId, request);
  switch (operation) {
    case "sql_tx_execute": return execute(request.statement, true);
    case "sql_tx_query": return query(request.statement);
    case "sql_tx_session_create": return createSession(transaction, request);
    case "sql_tx_session_capture":
      return { changeset: captureChangeset(transaction, request) };
    case "sql_tx_changeset_apply": return applyChangeset(request);
    case "sql_tx_commit":
      removeAuthorizer();
      removeOperationContext();
      deleteSessions(transaction);
      allowManagedTransactionControl = true;
      try {
        db.exec("COMMIT");
      } finally {
        allowManagedTransactionControl = false;
      }
      activeTransaction = null;
      return null;
    case "sql_tx_rollback":
      clearTransaction(transaction, { rollback: true });
      return null;
    default: throw new Error(`unsupported SQLite transaction operation: ${operation}`);
  }
}

function dispatch(operation, request, clientId) {
  if (operation.startsWith("sql_tx_")) return dispatchTransaction(operation, request, clientId);
  if (activeTransaction) {
    const error = new Error("SQLite is busy with a transaction owned by another request.");
    error.code = "busy";
    throw error;
  }
  switch (operation) {
    case "store_get": return storeGet(request);
    case "store_contains": return storeContains(request);
    case "store_set": storeSet(request); return null;
    case "store_remove": return storeRemove(request);
    case "store_batch": return storeBatch(request);
    case "store_list_prefix": return storeListPrefix(request);
    case "sql_execute": return execute(request, true);
    case "sql_query": return query(request.statement);
    case "sql_transaction": return sqlTransaction(request);
    case "sql_migrate": return sqlMigrate(request);
    default: throw new Error(`unsupported SQLite operation: ${operation}`);
  }
}

function errorResponse(error) {
  const sqliteCode = Number.isInteger(error?.resultCode) ? error.resultCode : undefined;
  let code = error?.code;
  if (!code && sqliteCode !== undefined) {
    const primary = sqliteCode & 0xff;
    if (primary === 5 || primary === 6) code = "busy";
    else if (primary === 13) code = "quota_exceeded";
    else if (primary === 10 || primary === 14) code = "unavailable";
    else code = "sqlite";
  }
  if (!code && error?.name === "QuotaExceededError") code = "quota_exceeded";
  if (!code) code = "invalid_request";
  return {
    code,
    sqlite_code: sqliteCode,
    message: error instanceof Error ? error.message : String(error),
  };
}

function handleMessage(endpoint, data) {
  let operation;
  let request;
  try {
    ({ operation, request } = data.request || {});
    if (typeof operation !== "string" || !request || typeof request !== "object") {
      const error = new Error("invalid SQLite worker request");
      error.code = "invalid_request";
      throw error;
    }
    endpoint.postMessage({
      id: data.id,
      ok: true,
      value: dispatch(operation, request, data.clientId),
    });
  } catch (error) {
    const transaction = activeTransaction;
    const isTransactionOperation = typeof operation === "string" && operation.startsWith("sql_tx_");
    const ownsFailedTransaction = isTransactionOperation &&
      operation !== "sql_tx_begin" && transaction?.clientId === data.clientId &&
      transaction?.id === request?.transaction_id;
    if (ownsFailedTransaction) clearTransaction(transaction, { rollback: true });
    endpoint.postMessage({ id: data.id, ok: false, ...errorResponse(error) });
  }
}

function connect(endpoint) {
  installWorkerAuthorizer();
  endpoint.addEventListener("message", ({ data }) => handleMessage(endpoint, data));
  endpoint.start?.();
  endpoint.postMessage({ type: "ready", applicationId });
}

if (typeof SharedWorkerGlobalScope !== "undefined") {
  self.addEventListener("connect", ({ ports }) => connect(ports[0]));
} else {
  connect(self);
}
