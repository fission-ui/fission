import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { createWebTestAdapters } from "../src/web_test_control.mjs";

test("bridge factories run without dynamic code generation", () => {
  const result = spawnSync(process.execPath, [
    "--disallow-code-generation-from-strings", "--input-type=module", "-e",
    `await import(${JSON.stringify(new URL("../src/web_test_control.mjs", import.meta.url).href)});`,
  ], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
});

test("static adapters preserve command, frame, diagnostics and grown-memory ABI", async () => {
  const memory = new WebAssembly.Memory({ initial: 1 });
  const dispatched = [];
  let submitted = "";
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: (length) => {
      if (length > 128) return 0;
      submitted = new TextDecoder().decode(new Uint8Array(memory.buffer, 256, length));
      return 7;
    },
    fission_web_test_dispatch_command: (id) => {
      dispatched.push(id);
      return 0;
    },
    fission_web_test_poll_json_len: () => 2,
    fission_web_test_poll_json_ptr: () => 512,
    fission_web_test_frame_present: () => 1,
    fission_web_test_frame_width: () => 400,
    fission_web_test_frame_height: () => 300,
    fission_web_test_frame_number_high: () => 1,
    fission_web_test_frame_number_low: () => 2,
  };
  const adapters = createWebTestAdapters(exports, memory, 48);
  assert.equal(adapters.diagnosticsAddress, 64);
  assert.notEqual(adapters.diagnosticsAddress, 48, "diagnostics and polling use distinct memory locations");
  assert.throws(() => adapters.submit({}), TypeError);
  assert.equal(adapters.submit("x".repeat(129)), 0);
  assert.equal(adapters.submit('{"cmd":"Query"}'), 7);
  assert.equal(submitted, '{"cmd":"Query"}');
  assert.deepEqual(dispatched, [], "dispatch must not re-enter Wasm during submit");
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.deepEqual(dispatched, [7]);
  memory.grow(1);
  new Uint8Array(memory.buffer, 512, 2).set(new TextEncoder().encode("{}"));
  const view = new DataView(memory.buffer);
  view.setUint32(48, 3, true);
  view.setUint32(64, 4, true);
  view.setUint32(68, 5, true);
  view.setUint32(72, 6, true);
  assert.equal(adapters.poll(7), "{}");
  assert.equal(adapters.pollDiagnostics(), 3);
  assert.deepEqual(adapters.diagnostics(), {
    stage: 4,
    stackPointer: 5,
    iteration: 6,
    bridgeDispatch: {
      attempts: 1,
      completions: 1,
      failures: 0,
      lastRequestId: 7,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      deferredWakeSchedules: 0,
      deferredWakeCalls: 0,
      deferredWakeFailures: 0,
    },
  });
  assert.deepEqual(JSON.parse(adapters.frame()), {
    width: 400, height: 300, frame: 4294967298, phase: "submitted",
  });
});

test("poll stays observational until the deferred command dispatch runs", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const dispatched = [];
  const wakeEvents = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  new Uint8Array(memory.buffer, 512, 4).set(new TextEncoder().encode("null"));
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => 118,
    fission_web_test_dispatch_command: (id) => {
      dispatched.push(id);
      new Uint8Array(memory.buffer, 512, 2).set(new TextEncoder().encode("{}"));
      return 1;
    },
    fission_web_test_wake_event_loop: () => wakeEvents.push("wake"),
    fission_web_test_deferred_wake_scheduled: () => wakeEvents.push("scheduled"),
    fission_web_test_poll_json_len: () => dispatched.length === 0 ? 4 : 2,
    fission_web_test_poll_json_ptr: () => 512,
  };
  globalThis.setTimeout = (callback) => {
    timers.push(callback);
    return 1;
  };

  try {
    const adapters = createWebTestAdapters(exports, memory, 48);
    const requestId = adapters.submit('{"cmd":"CaptureScreenshot"}');

    assert.equal(requestId, 118);
    assert.equal(adapters.poll(requestId), "null");
    assert.deepEqual(dispatched, [], "polling must not enter the event loop synchronously");
    assert.equal(timers.length, 2, "poll must schedule a deferred retry for a pending request");
    assert.deepEqual(adapters.diagnostics().bridgeDispatch.pendingRequestIds, [requestId]);

    // Simulate the submit timer being missed: the deferred retry still dispatches.
    timers[1]();
    assert.deepEqual(dispatched, [requestId], "the timer must not dispatch twice");
    assert.deepEqual(wakeEvents, ["scheduled"], "the wake must be scheduled after dispatch returns");
    assert.equal(timers.length, 3, "capture dispatch must schedule a separate wake task");
    timers[2]();
    assert.deepEqual(wakeEvents, ["scheduled", "wake"], "the wake export runs only in its timer task");
    assert.deepEqual(adapters.diagnostics().bridgeDispatch, {
      attempts: 1,
      completions: 1,
      failures: 0,
      lastRequestId: requestId,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      deferredWakeSchedules: 1,
      deferredWakeCalls: 1,
      deferredWakeFailures: 0,
    });
    assert.equal(adapters.poll(requestId), "{}");
    timers[0]();
    assert.deepEqual(dispatched, [requestId], "the original timer must be idempotent");
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});

test("capture wake remains deferred after hundreds of requests", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  let nextRequestId = 0;
  let dispatchCount = 0;
  let scheduledWakeCount = 0;
  let wakeCount = 0;
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => ++nextRequestId,
    fission_web_test_dispatch_command: () => {
      dispatchCount += 1;
      return 1;
    },
    fission_web_test_wake_event_loop: () => { wakeCount += 1; },
    fission_web_test_deferred_wake_scheduled: () => { scheduledWakeCount += 1; },
    fission_web_test_poll_json_len: () => 4,
    fission_web_test_poll_json_ptr: () => 512,
  };
  globalThis.setTimeout = (callback) => {
    timers.push(callback);
    return timers.length;
  };

  try {
    const adapters = createWebTestAdapters(exports, memory, 48);
    for (let index = 0; index < 600; index += 1) {
      adapters.submit('{"cmd":"CaptureScreenshot"}');
      const dispatch = timers.shift();
      assert.equal(typeof dispatch, "function");
      dispatch();
      assert.equal(wakeCount, index, "wake must not run on the dispatch stack");
      const wake = timers.shift();
      assert.equal(typeof wake, "function");
      wake();
    }

    assert.equal(dispatchCount, 600);
    assert.equal(scheduledWakeCount, 600);
    assert.equal(wakeCount, 600);
    assert.deepEqual(adapters.diagnostics().bridgeDispatch, {
      attempts: 600,
      completions: 600,
      failures: 0,
      lastRequestId: 600,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      deferredWakeSchedules: 600,
      deferredWakeCalls: 600,
      deferredWakeFailures: 0,
    });
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});
