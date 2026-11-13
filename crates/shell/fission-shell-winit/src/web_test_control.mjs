// Static module factories work under a CSP that disallows eval/new Function.
// Keep the bridge's raw-memory ABI unchanged: factories close over exports,
// not a Wasm callback whose stack may already have trapped.
export function createWebTestAdapters(exports, memory, pollStageAddress) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const diagnosticsAddress = exports.fission_web_test_diagnostics_ptr() >>> 0;
  return {
    diagnosticsAddress,
    submit(commandJson) {
      if (typeof commandJson !== "string") {
        throw new TypeError("command JSON must be a string");
      }
      const bytes = encoder.encode(commandJson);
      const capacity = exports.fission_web_test_command_json_capacity() >>> 0;
      if (bytes.length > capacity) {
        return exports.fission_web_test_submit_json(capacity + 1) >>> 0;
      }
      const address = exports.fission_web_test_command_json_ptr() >>> 0;
      new Uint8Array(memory.buffer, address, bytes.length).set(bytes);
      const requestId = exports.fission_web_test_submit_json(bytes.length) >>> 0;
      if (requestId !== 0) {
        globalThis.setTimeout(() => exports.fission_web_test_dispatch_command(requestId), 0);
      }
      return requestId;
    },
    poll(requestId) {
      const length = exports.fission_web_test_poll_json_len(requestId) >>> 0;
      const address = exports.fission_web_test_poll_json_ptr() >>> 0;
      return decoder.decode(new Uint8Array(memory.buffer, address, length));
    },
    pollDiagnostics() {
      return new DataView(memory.buffer).getUint32(pollStageAddress, true);
    },
    frame() {
      if (!exports.fission_web_test_frame_present()) return "null";
      const frame = (exports.fission_web_test_frame_number_high() >>> 0) * 4294967296
        + (exports.fission_web_test_frame_number_low() >>> 0);
      return JSON.stringify({
        width: exports.fission_web_test_frame_width(),
        height: exports.fission_web_test_frame_height(),
        frame,
        phase: "submitted",
      });
    },
    diagnostics() {
      const view = new DataView(memory.buffer);
      return {
        stage: view.getUint32(diagnosticsAddress, true),
        stackPointer: view.getUint32(diagnosticsAddress + 4, true),
        iteration: view.getUint32(diagnosticsAddress + 8, true),
      };
    },
  };
}
