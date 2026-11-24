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
const temporaryDirectoryName = `fission-tmp-${databaseId || "default"}`;

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
async function databaseDirectory(create = true) {
  const parts = databaseFilename.split("/").filter(Boolean);
  parts.pop();
  let directory = await navigator.storage.getDirectory();
  for (const part of parts) directory = await directory.getDirectoryHandle(part, { create });
  return directory;
}

async function cleanTemporaryFiles() {
  const parent = await databaseDirectory();
  let directory;
  try {
    directory = await parent.getDirectoryHandle(temporaryDirectoryName);
  } catch (error) {
    if (error?.name === "NotFoundError") return;
    throw error;
  }
  for await (const [name] of directory.entries()) {
    await directory.removeEntry(name, { recursive: true });
  }
}

// Only this database's private session files are considered. The database
// worker lock guarantees no live session for this named DB can be using them.
await cleanTemporaryFiles();
const db = new sqlite3.oo1.OpfsWlDb(databaseFilename, "c");
let activeTransaction = null;
let activeAuthorizerPolicy = null;
let allowManagedTransactionControl = false;
let allowManagedAttach = false;
let maintenance = null;
const snapshotSessions = new Map();
const importSessions = new Map();
const stagedDatabases = new Map();
const activeAttachedStagePaths = new Set();
const activeReadOnlySchemas = new Set();

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

function query(statement, database = db, readOnly = false) {
  const prepared = database.prepare(statement.sql);
  try {
    if (readOnly && sqlite3.capi.sqlite3_stmt_readonly(prepared.pointer) !== 1) {
      const error = new Error("staged SQLite queries must be read-only");
      error.code = "read_only";
      throw error;
    }
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

function temporaryFilePath() {
  return `/fission-apps/${namespace}/${temporaryDirectoryName}/${crypto.randomUUID()}.sqlite3`;
}

async function ensureTemporaryDirectory() {
  const parent = await databaseDirectory();
  await parent.getDirectoryHandle(temporaryDirectoryName, { create: true });
}

async function opfsFileHandle(filename, create = false) {
  const parts = filename.split("/").filter(Boolean);
  const basename = parts.pop();
  let directory = await navigator.storage.getDirectory();
  for (const part of parts) directory = await directory.getDirectoryHandle(part, { create });
  return { directory, file: await directory.getFileHandle(basename, { create }) };
}

async function removeOpfsFile(filename) {
  const parts = filename.split("/").filter(Boolean);
  const basename = parts.pop();
  let directory = await navigator.storage.getDirectory();
  try {
    for (const part of parts) directory = await directory.getDirectoryHandle(part);
  } catch (error) {
    if (error?.name === "NotFoundError") return;
    throw error;
  }
  for await (const [name] of directory.entries()) {
    if (name === basename || name.startsWith(`${basename}-`)) {
      await directory.removeEntry(name, { recursive: true });
    }
  }
}

function backupError(database, code, action) {
  const capi = sqlite3.capi;
  const message = capi.sqlite3_errmsg(database.pointer);
  const error = new Error(`SQLite ${action} failed (${code}): ${message}`);
  error.resultCode = code;
  return error;
}

async function backupDatabase(source, destination, shouldCancel = () => false) {
  const capi = sqlite3.capi;
  const backup = capi.sqlite3_backup_init(destination.pointer, "main", source.pointer, "main");
  if (!backup) throw backupError(destination, capi.sqlite3_errcode(destination.pointer), "backup initialization");
  let completed = false;
  let busySince = null;
  let busyRetries = 0;
  const startedAt = Date.now();
  try {
    for (;;) {
      if (shouldCancel()) return false;
      if (Date.now() - startedAt >= 120_000) {
        const error = new Error("SQLite online backup exceeded its 120 second limit");
        error.code = "unavailable";
        throw error;
      }
      const result = capi.sqlite3_backup_step(backup, 128);
      if (result === capi.SQLITE_DONE) {
        completed = true;
        break;
      }
      if (result !== capi.SQLITE_OK && result !== capi.SQLITE_BUSY && result !== capi.SQLITE_LOCKED) {
        throw backupError(destination, result, "backup step");
      }
      if (result === capi.SQLITE_OK) {
        busySince = null;
        busyRetries = 0;
        await new Promise((resolve) => setTimeout(resolve, 0));
        continue;
      }
      busySince ??= Date.now();
      busyRetries += 1;
      if (Date.now() - busySince >= 5000 || busyRetries >= 100) {
        const error = backupError(destination, result, "backup retry");
        error.code = "busy";
        throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, Math.min(100, busyRetries * 10)));
    }
  } finally {
    const result = capi.sqlite3_backup_finish(backup);
    // SQLite rolls back the destination write transaction when finish runs
    // before SQLITE_DONE. Surface finish errors too: they can report a failed
    // rollback or a prior backup-step I/O failure.
    if (result !== capi.SQLITE_OK) throw backupError(destination, result, "backup finish");
  }
  return completed;
}

function verifyIntegrity(database) {
  const result = database.selectValue("PRAGMA quick_check");
  if (result !== "ok") {
    const error = new Error(`imported SQLite database failed quick_check: ${String(result)}`);
    error.code = "invalid_request";
    throw error;
  }
}

function busyError(message = "SQLite is busy with another worker operation.") {
  const error = new Error(message);
  error.code = "busy";
  return error;
}

function sessionError(message = "The SQLite session is unknown, expired, or belongs to another browser context.") {
  const error = new Error(message);
  error.code = "transaction_lost";
  return error;
}

function ownedSession(sessions, request, clientId) {
  const session = sessions.get(request.session_id);
  if (!session || session.clientId !== clientId) throw sessionError();
  return session;
}

async function beginSnapshot(clientId, request) {
  if (activeTransaction || maintenance) throw busyError();
  const task = maintenance = {
    kind: "snapshot-begin",
    clientId,
    beginToken: String(request.begin_token ?? ""),
    cancelled: false,
  };
  let path;
  let source;
  let destination;
  let keep = false;
  try {
    path = temporaryFilePath();
    await ensureTemporaryDirectory();
    if (task.cancelled) {
      const error = new Error("SQLite snapshot creation was cancelled");
      error.code = "unavailable";
      throw error;
    }
    source = new sqlite3.oo1.OpfsWlDb(databaseFilename, "c");
    destination = new sqlite3.oo1.OpfsWlDb(path, "c");
    const completed = await backupDatabase(source, destination, () => task.cancelled);
    if (!completed) {
      const error = new Error("SQLite snapshot creation was cancelled");
      error.code = "unavailable";
      throw error;
    }
    // sqlite3_backup_finish() releases the source read transaction, so there
    // is no transaction left to roll back here.
    source.close();
    source = null;
    destination.close();
    destination = null;
    const { file } = await opfsFileHandle(path);
    const access = await file.createSyncAccessHandle();
    let byteLength;
    try { byteLength = access.getSize(); } finally { access.close(); }
    if (task.cancelled) {
      const error = new Error("SQLite snapshot creation was cancelled");
      error.code = "unavailable";
      throw error;
    }
    if (!Number.isSafeInteger(byteLength) || byteLength <= 0) {
      throw new Error("SQLite snapshot has an invalid file size");
    }
    const sessionId = crypto.randomUUID();
    snapshotSessions.set(sessionId, {
      clientId,
      beginToken: task.beginToken,
      path,
      byteLength,
      offset: 0,
      readingStarted: false,
      busy: false,
      cancelRequested: false,
    });
    keep = true;
    return { session_id: sessionId, byte_len: String(byteLength) };
  } finally {
    try { source?.close(); } catch (_) { /* Snapshot cleanup. */ }
    try { destination?.close(); } catch (_) { /* Snapshot cleanup. */ }
    if (!keep && path) {
      try { await removeOpfsFile(path); } catch (_) { /* It may not have been created. */ }
    }
    if (maintenance === task) maintenance = null;
  }
}

async function snapshotQuery(request, clientId) {
  const session = ownedSession(snapshotSessions, request, clientId);
  if (session.readingStarted || session.busy) {
    const error = new Error("snapshot queries must run before chunk reads");
    error.code = "invalid_request";
    throw error;
  }
  session.busy = true;
  let snapshot;
  try {
    snapshot = new sqlite3.oo1.OpfsWlDb(session.path, "c");
    return query(request.query.statement, snapshot, true);
  } finally {
    try { snapshot?.close(); } catch (_) { /* Query cleanup. */ }
    session.busy = false;
  }
}

async function snapshotRead(request, clientId) {
  const session = ownedSession(snapshotSessions, request, clientId);
  const maxBytes = Number(request.max_bytes);
  if (!Number.isInteger(maxBytes) || maxBytes < 1 || maxBytes > 1024 * 1024) {
    const error = new Error("snapshot chunk size must be within 1..=1048576 bytes");
    error.code = "invalid_request";
    throw error;
  }
  if (session.busy) throw busyError("the snapshot is already being read");
  session.busy = true;
  let access;
  let done = false;
  try {
    session.readingStarted = true;
    const { file } = await opfsFileHandle(session.path);
    access = await file.createSyncAccessHandle();
    const size = Math.min(maxBytes, session.byteLength - session.offset);
    const bytes = new Uint8Array(size);
    const read = access.read(bytes, { at: session.offset });
    if (read !== size) throw new Error("OPFS returned a truncated SQLite snapshot chunk");
    session.offset += read;
    done = session.offset >= session.byteLength;
    if (done) snapshotSessions.delete(request.session_id);
    return { bytes, done };
  } finally {
    try { access?.close(); } finally {
      session.busy = false;
      if (done || session.cancelRequested) {
        snapshotSessions.delete(request.session_id);
        await removeOpfsFile(session.path);
      }
    }
  }
}

async function cleanupSnapshot(sessionId, clientId) {
  const session = snapshotSessions.get(sessionId);
  if (!session || session.clientId !== clientId) return;
  if (session.busy) {
    session.cancelRequested = true;
    return;
  }
  snapshotSessions.delete(sessionId);
  await removeOpfsFile(session.path);
}

async function beginImport(clientId, request) {
  if (activeTransaction || maintenance) throw busyError();
  const sessionId = crypto.randomUUID();
  const session = {
    clientId,
    beginToken: String(request.begin_token ?? ""),
    path: temporaryFilePath(),
    offset: 0,
    busy: true,
    cancelRequested: false,
  };
  importSessions.set(sessionId, session);
  let access;
  let success = false;
  try {
    await ensureTemporaryDirectory();
    if (session.cancelRequested) throw sessionError("SQLite import creation was cancelled");
    const { file } = await opfsFileHandle(session.path, true);
    access = await file.createSyncAccessHandle();
    if (session.cancelRequested) throw sessionError("SQLite import creation was cancelled");
    access.truncate(0);
    access.flush();
    success = true;
    return { session_id: sessionId };
  } finally {
    try { access?.close(); } finally {
      session.busy = false;
      if (!success || session.cancelRequested) {
        importSessions.delete(sessionId);
        try { await removeOpfsFile(session.path); } catch (_) { /* Partial creation cleanup. */ }
      }
    }
  }
}

async function importWrite(request, clientId) {
  const session = ownedSession(importSessions, request, clientId);
  const bytes = request.bytes instanceof Uint8Array
    ? request.bytes
    : new Uint8Array(request.bytes || []);
  if (bytes.length < 1 || bytes.length > 1024 * 1024) {
    const error = new Error("import chunks must contain 1..=1048576 bytes");
    error.code = "invalid_request";
    throw error;
  }
  if (session.busy) throw busyError("the import session is already being written");
  session.busy = true;
  try {
    const { file } = await opfsFileHandle(session.path);
    const access = await file.createSyncAccessHandle();
    let written;
    try {
      written = access.write(bytes, { at: session.offset });
      access.flush();
    } finally {
      access.close();
    }
    if (written !== bytes.length) throw new Error("OPFS wrote a truncated SQLite import chunk");
    session.offset += written;
    return { bytes_written: written };
  } finally {
    session.busy = false;
    if (session.cancelRequested) {
      importSessions.delete(request.session_id);
      await removeOpfsFile(session.path);
    }
  }
}

async function finishImport(request, clientId) {
  const session = ownedSession(importSessions, request, clientId);
  if (session.busy) throw busyError("the import session is busy");
  if (session.offset <= 0) {
    const error = new Error("an empty SQLite import cannot be staged");
    error.code = "invalid_request";
    throw error;
  }
  session.busy = true;
  let staged;
  try {
    staged = new sqlite3.oo1.OpfsWlDb(session.path, "c");
    verifyIntegrity(staged);
    staged.close();
    staged = null;
    importSessions.delete(request.session_id);
    stagedDatabases.set(request.session_id, {
      clientId,
      beginToken: session.beginToken,
      path: session.path,
      attached: new Set(),
      cancelRequested: false,
    });
    return { session_id: request.session_id };
  } catch (error) {
    importSessions.delete(request.session_id);
    try { await removeOpfsFile(session.path); } catch (_) { /* Preserve validation error. */ }
    throw error;
  } finally {
    try { staged?.close(); } catch (_) { /* Import cleanup. */ }
  }
}

async function cleanupImport(sessionId, clientId) {
  const session = importSessions.get(sessionId);
  if (!session || session.clientId !== clientId) return;
  if (session.busy) {
    session.cancelRequested = true;
    return;
  }
  importSessions.delete(sessionId);
  await removeOpfsFile(session.path);
}

async function stageQuery(request, clientId) {
  const session = ownedSession(stagedDatabases, request, clientId);
  let staged;
  try {
    staged = new sqlite3.oo1.OpfsWlDb(session.path, "c");
    return query(request.query.statement, staged, true);
  } finally {
    try { staged?.close(); } catch (_) { /* Stage query cleanup. */ }
  }
}

async function cleanupStage(sessionId, clientId) {
  const session = stagedDatabases.get(sessionId);
  if (!session || session.clientId !== clientId) return;
  if (session.attached.size > 0 || maintenance?.stageId === sessionId) {
    session.cancelRequested = true;
    if (maintenance?.stageId === sessionId) maintenance.cancelled = true;
    return;
  }
  stagedDatabases.delete(sessionId);
  await removeOpfsFile(session.path);
}

async function installStage(request, clientId, { requireEmpty = false } = {}) {
  const session = ownedSession(stagedDatabases, request, clientId);
  if (activeTransaction || maintenance || session.attached.size > 0) throw busyError();
  const task = maintenance = {
    kind: "stage-install",
    clientId,
    stageId: request.session_id,
    cancelled: false,
  };
  let staged;
  try {
    if (requireEmpty) {
      const count = Number(db.selectValue(`
        SELECT
          (SELECT count(*) FROM main.sqlite_schema
           WHERE substr(name, 1, 7) <> 'sqlite_'
             AND NOT (type='table' AND name='fission'))
          + (SELECT count(*) FROM main.fission)
      `));
      if (!Number.isSafeInteger(count) || count !== 0) {
        const error = new Error("target Web SQLite database is not empty");
        error.resultCode = sqlite3.capi.SQLITE_CONSTRAINT;
        throw error;
      }
    }
    staged = new sqlite3.oo1.OpfsWlDb(session.path, "c");
    const complete = await backupDatabase(staged, db, () => task.cancelled || session.cancelRequested);
    if (!complete) {
      const error = new Error("SQLite staged installation was cancelled");
      error.code = "unavailable";
      throw error;
    }
    staged.close();
    staged = null;
    stagedDatabases.delete(request.session_id);
    await removeOpfsFile(session.path);
    return null;
  } finally {
    try { staged?.close(); } catch (_) { /* Stage install cleanup. */ }
    if (maintenance === task) maintenance = null;
    if (session.cancelRequested && session.attached.size === 0) {
      stagedDatabases.delete(request.session_id);
      try { await removeOpfsFile(session.path); } catch (_) { /* May already be removed. */ }
    }
  }
}

async function cancelImportOrStage(sessionId, clientId) {
  if (importSessions.has(sessionId)) return cleanupImport(sessionId, clientId);
  return cleanupStage(sessionId, clientId);
}

async function cancelPendingSession(request, clientId) {
  const token = String(request.begin_token ?? "");
  if (!token) return;
  if (maintenance?.clientId === clientId && maintenance.beginToken === token) {
    maintenance.cancelled = true;
    return;
  }
  for (const [sessionId, session] of snapshotSessions) {
    if (session.clientId === clientId && session.beginToken === token) {
      await cleanupSnapshot(sessionId, clientId);
      return;
    }
  }
  for (const [sessionId, session] of importSessions) {
    if (session.clientId === clientId && session.beginToken === token) {
      await cleanupImport(sessionId, clientId);
      return;
    }
  }
  for (const [sessionId, session] of stagedDatabases) {
    if (session.clientId === clientId && session.beginToken === token) {
      await cleanupStage(sessionId, clientId);
      return;
    }
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

function sqlTransactionJob(steps) {
  if (!Array.isArray(steps)) {
    const error = new Error("SQLite transaction job steps must be an array.");
    error.code = "invalid_request";
    throw error;
  }
  const results = [];
  for (const [index, step] of steps.entries()) {
    const hasExecute = step && Object.hasOwn(step, "Execute");
    const hasQuery = step && Object.hasOwn(step, "Query");
    if (hasExecute === hasQuery) {
      const error = new Error(`SQLite transaction job step ${index} must be Execute or Query.`);
      error.code = "invalid_request";
      throw error;
    }
    if (hasExecute) results.push({ Execute: execute(step.Execute, true) });
    else results.push({ Query: query(step.Query) });
  }
  return results;
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
  // SQLITE_PRAGMA includes writes such as `PRAGMA user_version = ...` and
  // `writable_schema = ON`. It is never an implicit read permission. Callers
  // needing a pragma must name it explicitly with a Pragma rule.
  Read: [20, 21, 31],
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
  const schema = String(database ?? "").toLocaleLowerCase();
  if (allowManagedAttach && (action === authorizerActions.Attach || action === authorizerActions.Detach)) {
    return capi.SQLITE_OK;
  }
  if (action === authorizerActions.Attach && activeAttachedStagePaths.has(String(first ?? ""))) {
    return capi.SQLITE_DENY;
  }
  if (activeReadOnlySchemas.size > 0 &&
    (action === authorizerActions.Attach || action === authorizerActions.Detach)) {
    return capi.SQLITE_DENY;
  }
  if (activeReadOnlySchemas.has(schema) && [
    authorizerActions.CreateIndex, authorizerActions.CreateTable, authorizerActions.CreateTempIndex,
    authorizerActions.CreateTempTable, authorizerActions.CreateTempTrigger, authorizerActions.CreateTempView,
    authorizerActions.CreateTrigger, authorizerActions.CreateView, authorizerActions.Delete,
    authorizerActions.DropIndex, authorizerActions.DropTable, authorizerActions.DropTempIndex,
    authorizerActions.DropTempTable, authorizerActions.DropTempTrigger, authorizerActions.DropTempView,
    authorizerActions.DropTrigger, authorizerActions.DropView, authorizerActions.Insert,
    authorizerActions.Pragma, authorizerActions.Transaction, authorizerActions.Update,
    authorizerActions.Detach, authorizerActions.AlterTable, authorizerActions.Reindex,
    authorizerActions.Analyze, authorizerActions.CreateVtable, authorizerActions.DropVtable,
    authorizerActions.Savepoint,
  ].includes(action)) return capi.SQLITE_DENY;
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

async function clearTransaction(transaction, { rollback = false } = {}) {
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
  await detachAttachedStages(transaction);
}

async function detachAttachedStages(transaction) {
  if (!transaction?.attachedStages?.size) return;
  for (const [schema, stageId] of transaction.attachedStages) {
    try {
      await detachStagedAlias(transaction, schema, stageId);
    } catch (_) {
      // The worker will close the connection on shutdown; keep the stage alive
      // until that happens rather than deleting a file SQLite may still own.
      continue;
    }
  }
}

async function detachStagedAlias(transaction, schema, stageId) {
  const stage = stagedDatabases.get(stageId);
  allowManagedAttach = true;
  try {
    db.exec(`DETACH DATABASE "${schema}"`);
  } finally {
    allowManagedAttach = false;
  }
  activeReadOnlySchemas.delete(schema);
  stage?.attached.delete(schema);
  transaction.attachedStages.delete(schema);
  if (stage?.attached.size === 0) activeAttachedStagePaths.delete(stage.path);
  if (stage?.cancelRequested && stage.attached.size === 0) {
    await cleanupStage(stageId, stage.clientId);
  }
}

function currentlyAttachedStages() {
  const attached = new Map();
  for (const [stageId, stage] of stagedDatabases) {
    for (const schema of stage.attached) attached.set(schema, stageId);
  }
  return attached;
}

function attachStagedDatabase(transaction, request) {
  const stage = ownedSession(stagedDatabases, { session_id: request.stage_id }, transaction.clientId);
  const alias = String(request.alias ?? "");
  if (!/^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(alias) || ["main", "temp"].includes(alias.toLowerCase())) {
    const error = new Error("invalid staged database SQL alias");
    error.code = "invalid_request";
    throw error;
  }
  const schema = alias.toLowerCase();
  if (transaction.attachedStages.has(schema)) {
    const error = new Error("staged database alias is already attached");
    error.code = "invalid_request";
    throw error;
  }
  allowManagedAttach = true;
  try {
    db.exec({ sql: `ATTACH DATABASE ? AS "${alias}"`, bind: [stage.path] });
  } finally {
    allowManagedAttach = false;
  }
  transaction.attachedStages.set(schema, request.stage_id);
  stage.attached.add(schema);
  activeAttachedStagePaths.add(stage.path);
  activeReadOnlySchemas.add(schema);
  return null;
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

async function beginTransaction(clientId, request) {
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
    attachedStages: currentlyAttachedStages(),
  };
  activeTransaction = transaction;
  try {
    registerOperationContext(request.operationContext);
    installAuthorizer(request.authorizerPolicy);
  } catch (error) {
    await clearTransaction(transaction, { rollback: true });
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

async function dispatchTransaction(operation, request, clientId) {
  if (operation === "sql_tx_begin") return beginTransaction(clientId, request);
  const transaction = transactionFor(clientId, request);
  switch (operation) {
    case "sql_tx_execute": return execute(request.statement, true);
    case "sql_tx_query": return query(request.statement);
    case "sql_tx_job": return sqlTransactionJob(request.steps);
    case "sql_tx_session_create": return createSession(transaction, request);
    case "sql_tx_session_capture":
      return { changeset: captureChangeset(transaction, request) };
    case "sql_tx_changeset_apply": return applyChangeset(request);
    case "sql_tx_stage_attach": return attachStagedDatabase(transaction, request);
    case "sql_tx_stage_detach": {
      const alias = String(request.alias ?? "");
      if (!/^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(alias) || ["main", "temp"].includes(alias.toLowerCase())) {
        const error = new Error("invalid staged database SQL alias");
        error.code = "invalid_request";
        throw error;
      }
      const schema = alias.toLowerCase();
      const stageId = transaction.attachedStages.get(schema);
      if (!stageId) {
        const error = new Error("staged database alias is not attached to this transaction");
        error.code = "invalid_request";
        throw error;
      }
      // SQLite retains a read lock after this transaction reads the stage.
      // DETACH here would fail with SQLITE_LOCKED and roll back valid writes.
      // Every attached stage is already detached when this transaction ends;
      // acknowledge release now, keeping the alias alive until COMMIT/ROLLBACK.
      return null;
    }
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
      await detachAttachedStages(transaction);
      return null;
    case "sql_tx_rollback":
      await clearTransaction(transaction, { rollback: true });
      return null;
    default: throw new Error(`unsupported SQLite transaction operation: ${operation}`);
  }
}

function cancellationOperation(operation) {
  return operation === "sqlite_snapshot_cancel" ||
    operation === "sqlite_import_cancel" ||
    operation === "sqlite_stage_cancel" ||
    operation === "sqlite_session_cancel_pending";
}

function independentSessionOperation(operation) {
  return operation.startsWith("sqlite_snapshot_") ||
    operation.startsWith("sqlite_import_") ||
    operation === "sqlite_stage_query" ||
    operation === "sqlite_stage_cancel";
}

async function dispatch(operation, request, clientId) {
  if (maintenance && maintenance.kind !== "snapshot-begin" && !cancellationOperation(operation)) {
    throw busyError();
  }
  switch (operation) {
    case "sqlite_snapshot_begin": return beginSnapshot(clientId, request);
    case "sqlite_snapshot_query": return snapshotQuery(request, clientId);
    case "sqlite_snapshot_read": return snapshotRead(request, clientId);
    case "sqlite_snapshot_cancel": return cleanupSnapshot(request.session_id, clientId);
    case "sqlite_import_begin": return beginImport(clientId, request);
    case "sqlite_import_write": return importWrite(request, clientId);
    case "sqlite_import_finish_stage": return finishImport(request, clientId);
    case "sqlite_import_cancel": return cancelImportOrStage(request.session_id, clientId);
    case "sqlite_stage_query": return stageQuery(request, clientId);
    case "sqlite_stage_install": return installStage(request, clientId);
    case "sqlite_stage_install_empty": return installStage(request, clientId, { requireEmpty: true });
    case "sqlite_stage_cancel": return cleanupStage(request.session_id, clientId);
    case "sqlite_session_cancel_pending": return cancelPendingSession(request, clientId);
  }
  if (operation.startsWith("sql_tx_")) return dispatchTransaction(operation, request, clientId);
  if (activeTransaction && !independentSessionOperation(operation)) {
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

async function closeClient(clientId) {
  if (maintenance?.clientId === clientId) maintenance.cancelled = true;
  if (activeTransaction?.clientId === clientId) {
    await clearTransaction(activeTransaction, { rollback: true });
  }
  for (const [sessionId, session] of snapshotSessions) {
    if (session.clientId === clientId) await cleanupSnapshot(sessionId, clientId);
  }
  for (const [sessionId, session] of importSessions) {
    if (session.clientId === clientId) await cleanupImport(sessionId, clientId);
  }
  for (const [sessionId, session] of stagedDatabases) {
    if (session.clientId === clientId) await cleanupStage(sessionId, clientId);
  }
}

async function handleMessage(endpoint, data) {
  if (data?.type === "client-closed") {
    await closeClient(data.clientId);
    return;
  }
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
      value: await dispatch(operation, request, data.clientId),
    });
  } catch (error) {
    const transaction = activeTransaction;
    const isTransactionOperation = typeof operation === "string" && operation.startsWith("sql_tx_");
    const ownsFailedTransaction = isTransactionOperation &&
      operation !== "sql_tx_begin" && transaction?.clientId === data.clientId &&
      transaction?.id === request?.transaction_id;
    if (ownsFailedTransaction) await clearTransaction(transaction, { rollback: true });
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
