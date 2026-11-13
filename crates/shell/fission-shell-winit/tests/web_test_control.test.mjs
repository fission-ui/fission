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
    fission_web_test_dispatch_command: (id) => dispatched.push(id),
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
  assert.deepEqual(adapters.diagnostics(), { stage: 4, stackPointer: 5, iteration: 6 });
  assert.deepEqual(JSON.parse(adapters.frame()), {
    width: 400, height: 300, frame: 4294967298, phase: "submitted",
  });
});

test("poll dispatches a queued command if the timer has not run", () => {
  const originalSetTimeout = globalThis.setTimeout;
  const timers = [];
  const dispatched = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  new Uint8Array(memory.buffer, 512, 2).set(new TextEncoder().encode("{}"));
  const exports = {
    fission_web_test_diagnostics_ptr: () => 64,
    fission_web_test_command_json_ptr: () => 256,
    fission_web_test_command_json_capacity: () => 128,
    fission_web_test_submit_json: () => 118,
    fission_web_test_dispatch_command: (id) => dispatched.push(id),
    fission_web_test_poll_json_len: () => 2,
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
    assert.equal(adapters.poll(requestId), "{}");
    assert.deepEqual(dispatched, [requestId]);

    timers[0]();
    assert.deepEqual(dispatched, [requestId], "the timer must not dispatch twice");
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }
});
