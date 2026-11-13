import assert from "node:assert/strict";
import test from "node:test";
import { beginWhenAvailable, createHarness, deadline } from "./bridge-harness.mjs";

async function warm(page) {
  const transaction = await deadline(page.request("sql_tx_begin"));
  await deadline(page.request("sql_tx_rollback", transaction));
}

test("borrower close rolls back its transaction without killing the owner's worker", async (t) => {
  const harness = createHarness();
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  await warm(owner);
  const borrower = await harness.page("borrower");
  await deadline(borrower.request("sql_tx_begin"));
  const worker = harness.workers.find((worker) => !worker.terminated);
  const before = worker.commands.filter((command) => command === "ROLLBACK").length;
  assert.equal(borrower.close(), true);
  const next = await beginWhenAvailable(owner);
  assert.equal(worker.terminated, false);
  assert.equal(worker.commands.filter((command) => command === "ROLLBACK").length, before + 1);
  await owner.request("sql_tx_rollback", next);
});

test("abrupt borrower destruction releases its lifetime lock and transaction", async (t) => {
  const harness = createHarness();
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  await warm(owner);
  const borrower = await harness.page("borrower");
  await deadline(borrower.request("sql_tx_begin"));
  // No bridge close, disconnect message, Rust Drop, unload handler or heartbeat.
  borrower.destroy();
  const next = await beginWhenAvailable(owner);
  assert.equal(harness.workers.filter((worker) => !worker.terminated).length, 1);
  await owner.request("sql_tx_rollback", next);
});

test("closing a different client does not abort the transaction owner", async (t) => {
  const harness = createHarness();
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  await warm(owner);
  const borrower = await harness.page("borrower");
  const bystander = await harness.page("bystander");
  await warm(bystander);
  const transaction = await deadline(borrower.request("sql_tx_begin"));
  assert.equal(bystander.close(), true);
  await assert.rejects(owner.request("sql_tx_begin"), { code: "busy" });
  await deadline(borrower.request("sql_tx_rollback", transaction));
  await warm(owner);
});

test("closing while BEGIN is in flight cannot strand a transaction", async (t) => {
  const harness = createHarness();
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  await warm(owner);
  const borrower = await harness.page("borrower");
  await warm(borrower);
  const pending = assert.rejects(borrower.request("sql_tx_begin"), /closed/);
  assert.equal(borrower.close(), true);
  await deadline(pending);
  const next = await beginWhenAvailable(owner);
  await owner.request("sql_tx_rollback", next);
});

test("Read permission never implicitly permits a modifying PRAGMA", async (t) => {
  const harness = createHarness();
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  const transaction = await deadline(owner.request("sql_tx_begin", {
    authorizerPolicy: { defaultDecision: "Deny", rules: [{ action: "Read", decision: "Allow" }] },
  }));
  await assert.rejects(owner.request("sql_tx_execute", {
    ...transaction, statement: { sql: "PRAGMA user_version = 99", parameters: "None" },
  }), { sqliteCode: 23 });
  await warm(owner);
});

test("an explicitly named PRAGMA rule can authorize intentional metadata changes", async (t) => {
  const harness = createHarness();
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  const transaction = await deadline(owner.request("sql_tx_begin", {
    authorizerPolicy: {
      defaultDecision: "Deny",
      rules: [{ action: "Pragma", decision: "Allow", firstArgument: "user_version", secondArgument: "99" }],
    },
  }));
  await deadline(owner.request("sql_tx_execute", {
    ...transaction, statement: { sql: "PRAGMA user_version = 99", parameters: "None" },
  }));
  await deadline(owner.request("sql_tx_commit", transaction));
});

test("worker startup failure rejects waiting borrowers and later requests within a bound", async (t) => {
  const harness = createHarness({ workerStartupFailure: true });
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  const borrower = await harness.page("borrower");
  await deadline(Promise.all([
    assert.rejects(owner.request("sql_tx_begin"), { code: "unavailable" }),
    assert.rejects(borrower.request("sql_tx_begin"), { code: "unavailable" }),
  ]));
  await deadline(assert.rejects(owner.request("sql_tx_begin"), { code: "unavailable" }));
  await deadline(assert.rejects(borrower.request("sql_tx_begin"), { code: "unavailable" }));
  assert.equal(harness.workers.length, 4); // Initial attempt plus three bounded restarts.
});

test("a synchronous Worker construction error does not strand requests", async (t) => {
  const harness = createHarness({ workerConstructionFailure: true });
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  await deadline(assert.rejects(owner.request("sql_tx_begin"), { code: "unavailable" }));
  const lateBorrower = await harness.page("late borrower");
  await deadline(assert.rejects(lateBorrower.request("sql_tx_begin"), { code: "unavailable" }));
});

test("a failed lifetime lock remains unavailable after owner-ready announcements", async (t) => {
  const harness = createHarness({ denyLifetimeLock: true });
  t.after(() => harness.close());
  const owner = await harness.page("owner");
  await deadline(assert.rejects(owner.request("sql_tx_begin"), { code: "unavailable" }));
  await new Promise((resolve) => setTimeout(resolve, 10));
  await deadline(assert.rejects(owner.request("sql_tx_begin"), { code: "unavailable" }));
});
