import assert from "node:assert/strict";
import test from "node:test";
import { createHarness, deadline } from "./bridge-harness.mjs";

for (const completion of ["commit", "rollback"]) {
  test(`stage release waits until ${completion} after the stage has been read`, async (t) => {
    const harness = createHarness({ lockStagesDuringTransaction: true });
    t.after(() => harness.close());
    const page = await harness.page(`stage release ${completion}`);
    const pending = await deadline(page.request("sqlite_import_begin", { begin_token: completion }));
    await deadline(page.request("sqlite_import_write", { ...pending, bytes: [1, 2, 3] }));
    const stage = await deadline(page.request("sqlite_import_finish_stage", pending));
    const transaction = await deadline(page.request("sql_tx_begin"));
    await deadline(page.request("sql_tx_stage_attach", {
      ...transaction, stage_id: stage.session_id, alias: "incoming",
    }));
    await deadline(page.request("sql_tx_execute", {
      ...transaction, statement: { sql: "INSERT INTO main.records SELECT * FROM incoming.records" },
    }));
    await deadline(page.request("sql_tx_stage_detach", { ...transaction, alias: "incoming" }));
    await deadline(page.request("sql_tx_stage_detach", { ...transaction, alias: "incoming" }));
    const commands = harness.workers[0].commands;
    assert.ok(!commands.some((command) => command.startsWith("DETACH")));
    await deadline(page.request(`sql_tx_${completion}`, transaction));
    assert.ok(commands.indexOf(`${completion.toUpperCase()}`) < commands.indexOf("DETACH DATABASE"));
    await deadline(page.request("sqlite_stage_cancel", stage));
    assert.equal(harness.opfs.listFiles().length, 0);
  });
}

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
  assert.equal(result?.code, "transaction_lost", `begin returned ${JSON.stringify(result)}`);
  assert.equal(harness.opfs.listFiles(partialPath).length, 0);
});
