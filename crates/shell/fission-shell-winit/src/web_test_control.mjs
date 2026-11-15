// Static module factories work under a CSP that disallows eval/new Function.
// Keep the bridge's raw-memory ABI unchanged: factories close over exports,
// not a Wasm callback whose stack may already have trapped.
export function createWebTestAdapters(exports, memory, pollStageAddress) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const diagnosticsAddress = exports.fission_web_test_diagnostics_ptr() >>> 0;
  const pendingDispatches = new Set();
  const dispatchesInFlight = new Set();
  let dispatchAttempts = 0;
  let dispatchCompletions = 0;
  let dispatchFailures = 0;
  let lastDispatchRequestId = 0;
  let deferredWakeSchedules = 0;
  let deferredWakeCalls = 0;
  let deferredWakeExportReturns = 0;
  let deferredWakeSendSuccesses = 0;
  let lastDeferredWakeSendResult = null;
  let deferredWakeFailures = 0;

  const dispatch = (requestId) => {
    if (!pendingDispatches.has(requestId) || dispatchesInFlight.has(requestId)) return;
    dispatchesInFlight.add(requestId);
    dispatchAttempts += 1;
    lastDispatchRequestId = requestId;
    let wakeAfterDispatch = false;
    try {
      wakeAfterDispatch = (exports.fission_web_test_dispatch_command(requestId) >>> 0) === 1;
      pendingDispatches.delete(requestId);
      dispatchCompletions += 1;
    } catch (error) {
      pendingDispatches.delete(requestId);
      dispatchFailures += 1;
      throw error;
    } finally {
      dispatchesInFlight.delete(requestId);
    }
    if (wakeAfterDispatch) {
      // Run the extra Winit wake only after the command's Wasm export has
      // returned. A browser-owned timer avoids retaining a Rust Closure for
      // the deferred event and cannot re-enter the shell during dispatch.
      try {
        globalThis.setTimeout(() => {
          deferredWakeCalls += 1;
          try {
            // A scheduled callback is not evidence that the Wasm wake export
            // ran or that Winit accepted its proxy event. Keep all three
            // boundaries visible to LiveTest diagnostics.
            const sent = (exports.fission_web_test_wake_event_loop() >>> 0) === 1;
            deferredWakeExportReturns += 1;
            lastDeferredWakeSendResult = sent;
            if (sent) deferredWakeSendSuccesses += 1;
          } catch (error) {
            deferredWakeFailures += 1;
            throw error;
          }
        }, 0);
        deferredWakeSchedules += 1;
        exports.fission_web_test_deferred_wake_scheduled();
      } catch (error) {
        deferredWakeFailures += 1;
        throw error;
      }
    }
  };

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
        pendingDispatches.add(requestId);
        globalThis.setTimeout(() => dispatch(requestId), 0);
      }
      return requestId;
    },
    poll(requestId) {
      // Runtime.evaluate polls can overtake a queued zero-delay timer. Retry
      // dispatch on a later task while the request is still pending, but never
      // enter Wasm from inside poll: the CDP response must return first.
      if (pendingDispatches.has(requestId)) {
        globalThis.setTimeout(() => dispatch(requestId), 0);
      }
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
        bridgeDispatch: {
          attempts: dispatchAttempts,
          completions: dispatchCompletions,
          failures: dispatchFailures,
          lastRequestId: lastDispatchRequestId,
          pendingRequestIds: [...pendingDispatches],
          inFlightRequestIds: [...dispatchesInFlight],
          deferredWakeSchedules,
          deferredWakeCalls,
          deferredWakeExportReturns,
          deferredWakeSendSuccesses,
          lastDeferredWakeSendResult,
          deferredWakeFailures,
        },
      };
    },
  };
}
