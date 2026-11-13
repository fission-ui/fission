import assert from "node:assert/strict";
import test from "node:test";

import { createWebTestAdapters } from "./web_test_control.mjs";

function makeAdapters(dispatches) {
  const memory = { buffer: new ArrayBuffer(256) };
  const response = new TextEncoder().encode("null");
  new Uint8Array(memory.buffer, 32, response.length).set(response);

  const exports = {
    fission_web_test_diagnostics_ptr: () => 128,
    fission_web_test_command_json_capacity: () => 64,
    fission_web_test_command_json_ptr: () => 0,
    fission_web_test_submit_json: () => 118,
    fission_web_test_dispatch_command: (requestId) => dispatches.push(requestId),
    fission_web_test_poll_json_len: () => response.length,
    fission_web_test_poll_json_ptr: () => 32,
  };

  return createWebTestAdapters(exports, memory, 120);
}

test("poll dispatches a queued command when its timer has not run", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const dispatches = [];
  globalThis.setTimeout = (callback) => {
    timers.push(callback);
    return 1;
  };

  try {
    const adapters = makeAdapters(dispatches);
    const requestId = adapters.submit("{}");

    assert.equal(requestId, 118);
    assert.equal(adapters.poll(requestId), "null");
    assert.deepEqual(dispatches, [requestId]);

    timers[0]();
    assert.deepEqual(dispatches, [requestId]);
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});

test("the deferred timer dispatches before polling without duplicate dispatch", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const dispatches = [];
  globalThis.setTimeout = (callback) => {
    timers.push(callback);
    return 1;
  };

  try {
    const adapters = makeAdapters(dispatches);
    const requestId = adapters.submit("{}");

    timers[0]();
    assert.equal(adapters.poll(requestId), "null");
    assert.deepEqual(dispatches, [requestId]);
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});
