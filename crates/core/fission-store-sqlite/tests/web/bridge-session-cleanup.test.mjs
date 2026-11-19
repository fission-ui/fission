import assert from "node:assert/strict";
import test from "node:test";
import { createHarness, deadline } from "./bridge-harness.mjs";

test("disconnect during import begin removes its partial OPFS file", async (t) => {
  let releaseAccess;
  let accessStarted;
  let clientClosed;
  const accessGate = new Promise((resolve) => { releaseAccess = resolve; });
  const started = new Promise((resolve) => { accessStarted = resolve; });
  const closed = new Promise((resolve) => { clientClosed = resolve; });
  const harness = createHarness({
    beforeOpfsAccess: async (path) => {
      if (!path.includes("fission-tmp-")) return;
      accessStarted(path);
      await accessGate;
    },
    onWorkerClientClosed: clientClosed,
  });
  t.after(() => harness.close());

  const owner = await harness.page("owner");
  const warm = await deadline(owner.request("sql_tx_begin"));
  await deadline(owner.request("sql_tx_rollback", warm));
  const borrower = await harness.page("borrower");
  const pendingBegin = borrower
    .request("sqlite_import_begin", { begin_token: "disconnect-test" })
    .catch(() => null);
  const partialPath = await deadline(started);

  borrower.destroy();
  await deadline(closed);
  releaseAccess();

  const until = Date.now() + 2000;
  while (harness.opfs.listFiles(partialPath).length > 0 && Date.now() < until) {
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  assert.equal(harness.opfs.listFiles(partialPath).length, 0);

  const next = await deadline(owner.request("sql_tx_begin"));
  await deadline(owner.request("sql_tx_rollback", next));
  void pendingBegin;
});

test("cancelling an in-flight import begin removes its partial OPFS file", async (t) => {
  let releaseAccess;
  let accessStarted;
  const accessGate = new Promise((resolve) => { releaseAccess = resolve; });
  const started = new Promise((resolve) => { accessStarted = resolve; });
  const harness = createHarness({
    beforeOpfsAccess: async (path) => {
      if (!path.includes("fission-tmp-")) return;
      accessStarted(path);
      await accessGate;
    },
  });
  t.after(() => harness.close());
  const page = await harness.page("cancel owner");
  const pendingBegin = page
    .request("sqlite_import_begin", { begin_token: "cancel-test" })
    .catch((error) => error);
  const partialPath = await deadline(started);
  await deadline(page.request("sqlite_session_cancel_pending", { begin_token: "cancel-test" }));
  releaseAccess();
  const result = await deadline(pendingBegin);
  assert.ok(result instanceof Error);
  assert.equal(harness.opfs.listFiles(partialPath).length, 0);
});
