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
      return 2;
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
    captureRequestsPending: null,
    bridgeDispatch: {
      attempts: 1,
      completions: 1,
      failures: 0,
      lastRequestId: 7,
      lastStatus: 2,
      accepted: 1,
      unacknowledged: 0,
      lastAccepted: true,
      lastWakeRequested: false,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      pendingResponseWakeCount: 0,
      lastResponseWakeRequestId: null,
      lastResponsePollPending: null,
      deferredWakeSchedules: 0,
      deferredWakeCalls: 0,
      deferredWakeExportReturns: 0,
      deferredWakeSendSuccesses: 0,
      lastDeferredWakeSendResult: null,
      deferredWakeFailures: 0,
      deferredWakeRetrySchedules: 0,
      deferredWakeRetryCalls: 0,
    },
  });
  assert.deepEqual(JSON.parse(adapters.frame()), {
    width: 400, height: 300, frame: 4294967298, phase: "submitted",
  });
});

test("an unacknowledged command remains queued until a deferred retry accepts it", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  let dispatchAttempts = 0;
  let response = "null";
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => 9,
    fission_web_test_dispatch_command: () => {
      dispatchAttempts += 1;
      if (dispatchAttempts === 1) return 0;
      response = "{}";
      return 2;
    },
    fission_web_test_poll_json_len: () => {
      const bytes = new TextEncoder().encode(response);
      new Uint8Array(memory.buffer, 512, bytes.length).set(bytes);
      return bytes.length;
    },
    fission_web_test_poll_json_ptr: () => 512,
  };
  globalThis.setTimeout = (callback) => {
    timers.push(callback);
    return timers.length;
  };

  try {
    const adapters = createWebTestAdapters(exports, memory, 48);
    const requestId = adapters.submit('{"cmd":"Query"}');
    assert.equal(requestId, 9);

    timers.shift()();
    assert.equal(dispatchAttempts, 1);
    assert.equal(adapters.poll(requestId), "null");
    assert.deepEqual(adapters.diagnostics().bridgeDispatch.pendingRequestIds, [requestId]);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastStatus, 0);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastAccepted, false);
    assert.equal(adapters.diagnostics().bridgeDispatch.unacknowledged, 1);
    assert.equal(timers.length, 1, "poll schedules a new deferred dispatch attempt");

    timers.shift()();
    assert.equal(dispatchAttempts, 2);
    assert.deepEqual(adapters.diagnostics().bridgeDispatch.pendingRequestIds, []);
    assert.equal(adapters.poll(requestId), "{}");
    assert.equal(adapters.diagnostics().bridgeDispatch.lastStatus, 2);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastAccepted, true);
    assert.equal(adapters.diagnostics().bridgeDispatch.accepted, 1);
    assert.equal(timers.length, 0);
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
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
      return 3;
    },
    fission_web_test_wake_event_loop: () => {
      wakeEvents.push("wake");
      return 1;
    },
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
      lastStatus: 3,
      accepted: 1,
      unacknowledged: 0,
      lastAccepted: true,
      lastWakeRequested: true,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      pendingResponseWakeCount: 1,
      lastResponseWakeRequestId: requestId,
      lastResponsePollPending: true,
      deferredWakeSchedules: 1,
      deferredWakeCalls: 1,
      deferredWakeExportReturns: 1,
      deferredWakeSendSuccesses: 1,
      lastDeferredWakeSendResult: true,
      deferredWakeFailures: 0,
      deferredWakeRetrySchedules: 0,
      deferredWakeRetryCalls: 0,
    });
    assert.equal(adapters.poll(requestId), "{}");
    timers[0]();
    assert.deepEqual(dispatched, [requestId], "the original timer must be idempotent");
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});

test("a pending selector response retries a deferred event-loop wake", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const timers = [];
  const dispatched = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  let responseJson = null;
  let wakeCalls = 0;
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => 119,
    fission_web_test_dispatch_command: (id) => {
      dispatched.push(id);
      return 3;
    },
    fission_web_test_wake_event_loop: () => {
      wakeCalls += 1;
      // Model EventLoopProxy accepting the query while Winit misses its
      // callback once; the retry lets the pending selector request complete.
      if (wakeCalls === 2) responseJson = JSON.stringify({ status: "Ok" });
      return 1;
    },
    fission_web_test_deferred_wake_scheduled: () => {},
    fission_web_test_poll_json_len: () => {
      const bytes = new TextEncoder().encode(responseJson ?? "null");
      new Uint8Array(memory.buffer, 512, bytes.length).set(bytes);
      return bytes.length;
    },
    fission_web_test_poll_json_ptr: () => 512,
  };
  globalThis.setTimeout = (callback, delay = 0) => {
    const timer = { callback, delay, cleared: false };
    timers.push(timer);
    return timer;
  };
  globalThis.clearTimeout = (timer) => { timer.cleared = true; };

  try {
    const adapters = createWebTestAdapters(exports, memory, 48);
    const requestId = adapters.submit('{"cmd":"TapSelector","query":"browse"}');
    assert.equal(requestId, 119);

    timers.shift().callback(); // deferred command dispatch
    assert.deepEqual(dispatched, [requestId]);
    assert.equal(wakeCalls, 0, "the wake must not run on the Wasm dispatch stack");
    assert.equal(adapters.poll(requestId), "null");

    const firstWake = timers.shift();
    assert.equal(firstWake.delay, 0);
    firstWake.callback(); // accepted, but Winit misses this callback
    assert.equal(wakeCalls, 1);
    assert.equal(adapters.poll(requestId), "null");

    const retry = timers.shift();
    assert.equal(retry.delay, 16);
    retry.callback(); // the event loop handles the queued selector query
    assert.equal(wakeCalls, 2);
    assert.deepEqual(JSON.parse(adapters.poll(requestId)), { status: "Ok" });
    assert.equal(adapters.diagnostics().bridgeDispatch.pendingResponseWakeCount, 0);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastResponsePollPending, false);
    assert.equal(timers.length, 0, "response completion leaves no pending wake timer");
  } finally {
    globalThis.setTimeout = originalSetTimeout;
    globalThis.clearTimeout = originalClearTimeout;
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
      return 3;
    },
    fission_web_test_wake_event_loop: () => {
      wakeCount += 1;
      return 1;
    },
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
      lastStatus: 3,
      accepted: 600,
      unacknowledged: 0,
      lastAccepted: true,
      lastWakeRequested: true,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      pendingResponseWakeCount: 600,
      lastResponseWakeRequestId: 600,
      lastResponsePollPending: true,
      deferredWakeSchedules: 600,
      deferredWakeCalls: 600,
      deferredWakeExportReturns: 600,
      deferredWakeSendSuccesses: 600,
      lastDeferredWakeSendResult: true,
      deferredWakeFailures: 0,
      deferredWakeRetrySchedules: 0,
      deferredWakeRetryCalls: 0,
    });
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});

test("queued screenshot retries accepted wakes through queue drain and readback", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const timers = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  let queuePending = true;
  let wakeCalls = 0;
  let captureHandlerCalls = 0;
  let responseJson = null;
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => 687,
    fission_web_test_dispatch_command: () => 3,
    fission_web_test_wake_event_loop: () => {
      wakeCalls += 1;
      // Model EventLoopProxy::send_event returning Ok while Winit misses its
      // callback once. The second wake drains the queue, and a later pass
      // completes the asynchronous screenshot readback.
      if (wakeCalls === 2 && queuePending) {
        queuePending = false;
        captureHandlerCalls += 1;
      }
      if (wakeCalls === 3) {
        responseJson = JSON.stringify({
          status: "Screenshot",
          png_base64: "cG5n",
          width: 16,
          height: 9,
        });
      }
      return 1;
    },
    fission_web_test_capture_requests_pending: () => Number(queuePending),
    fission_web_test_deferred_wake_scheduled: () => {},
    fission_web_test_poll_json_len: () => {
      const bytes = new TextEncoder().encode(responseJson ?? "null");
      new Uint8Array(memory.buffer, 512, bytes.length).set(bytes);
      return bytes.length;
    },
    fission_web_test_poll_json_ptr: () => 512,
  };
  globalThis.setTimeout = (callback, delay = 0) => {
    const timer = { callback, delay, cleared: false };
    timers.push(timer);
    return timer;
  };
  globalThis.clearTimeout = (timer) => { timer.cleared = true; };

  try {
    const adapters = createWebTestAdapters(exports, memory, 48);
    const requestId = adapters.submit('{"cmd":"CaptureScreenshot"}');
    assert.equal(requestId, 687);

    timers.shift().callback(); // dispatch command
    timers.shift().callback(); // first accepted wake, callback is lost
    assert.equal(adapters.poll(requestId), "null");
    const retry = timers.shift();
    assert.equal(retry.delay, 16);
    retry.callback(); // second wake reaches AboutToWait and drains once
    assert.equal(adapters.diagnostics().captureRequestsPending, false);
    assert.equal(adapters.poll(requestId), "null");
    const readbackRetry = timers.shift();
    assert.equal(readbackRetry.delay, 32);
    readbackRetry.callback(); // third wake advances the screenshot readback

    const response = JSON.parse(adapters.poll(requestId));
    assert.equal(response.status, "Screenshot");
    assert.equal(response.png_base64, "cG5n");
    assert.equal(captureHandlerCalls, 1, "the queued request is drained once");
    assert.equal(wakeCalls, 3, "accepted wakes continue through screenshot readback");
    assert.equal(adapters.diagnostics().bridgeDispatch.deferredWakeRetrySchedules, 2);
    assert.equal(adapters.diagnostics().bridgeDispatch.deferredWakeRetryCalls, 2);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastStatus, 3);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastWakeRequested, true);
    assert.equal(adapters.diagnostics().bridgeDispatch.pendingResponseWakeCount, 0);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastResponseWakeRequestId, requestId);
    assert.equal(adapters.diagnostics().bridgeDispatch.lastResponsePollPending, false);
    assert.equal(adapters.poll(requestId), responseJson, "the response remains stable after polling");
    assert.equal(timers.length, 0, "response completion clears all wake retries");
  } finally {
    globalThis.setTimeout = originalSetTimeout;
    globalThis.clearTimeout = originalClearTimeout;
  }
});

test("wake diagnostics distinguish a timer callback from a rejected proxy send", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => 1,
    fission_web_test_dispatch_command: () => 3,
    fission_web_test_wake_event_loop: () => 0,
    fission_web_test_deferred_wake_scheduled: () => {},
  };
  globalThis.setTimeout = (callback) => {
    timers.push(callback);
    return timers.length;
  };

  try {
    const adapters = createWebTestAdapters(exports, memory, 48);
    adapters.submit('{"cmd":"CaptureScreenshot"}');
    timers.shift()();
    timers.shift()();

    assert.deepEqual(adapters.diagnostics().bridgeDispatch, {
      attempts: 1,
      completions: 1,
      failures: 0,
      lastRequestId: 1,
      lastStatus: 3,
      accepted: 1,
      unacknowledged: 0,
      lastAccepted: true,
      lastWakeRequested: true,
      pendingRequestIds: [],
      inFlightRequestIds: [],
      pendingResponseWakeCount: 1,
      lastResponseWakeRequestId: 1,
      lastResponsePollPending: true,
      deferredWakeSchedules: 1,
      deferredWakeCalls: 1,
      deferredWakeExportReturns: 1,
      deferredWakeSendSuccesses: 0,
      lastDeferredWakeSendResult: false,
      deferredWakeFailures: 0,
      deferredWakeRetrySchedules: 0,
      deferredWakeRetryCalls: 0,
    });
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});
