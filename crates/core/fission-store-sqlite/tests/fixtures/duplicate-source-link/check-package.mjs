// This is a binding/link smoke check, not a substitute for the real OPFS tests.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const packageDir = resolve(process.argv[2]);
const stem = "fission_store_sqlite_duplicate_source_link";
const calls = [];
globalThis.__fissionSqliteRequest = () => {
  throw new Error("the close-only binding fixture must not open a database");
};
globalThis.__fissionSqliteCloseDatabase = (name) => calls.push(name);

const bindings = await import(pathToFileURL(join(packageDir, `${stem}.js`)).href);
await bindings.default({
  module_or_path: await readFile(join(packageDir, `${stem}_bg.wasm`)),
});
bindings.close_both_sqlite_providers();
assert.deepEqual(calls, [undefined, undefined]);
console.log("Both SQLite source identities reached the packaged host bridge");
