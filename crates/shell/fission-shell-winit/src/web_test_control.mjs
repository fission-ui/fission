// Static module factories work under a CSP that disallows eval/new Function.
// Keep the bridge's raw-memory ABI unchanged: factories close over exports,
// not a Wasm callback whose stack may already have trapped.
export function createWebTestAdapters(exports, memory, pollStageAddress) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const diagnosticsAddress = exports.fission_web_test_diagnostics_ptr() >>> 0;
  const pendingDispatches = new Set();
  const dispatchesInFlight = new Set();
  const pendingCaptureWakes = new Set();
  const captureWakeTimers = new Map();
  const captureWakeRetryAttempts = new Map();
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
  let deferredWakeRetrySchedules = 0;
  let deferredWakeRetryCalls = 0;

  const captureWakeRetryDelay = (attempt) => [16, 32, 64, 128, 250][Math.min(attempt, 4)];
  const clearCaptureWake = (requestId) => {
    pendingCaptureWakes.delete(requestId);
    captureWakeRetryAttempts.delete(requestId);
    const timer = captureWakeTimers.get(requestId);
    if (timer !== undefined) globalThis.clearTimeout(timer);
    captureWakeTimers.delete(requestId);
  };
  const sendDeferredWake = (requestId, isRetry) => {
    if (captureWakeTimers.has(requestId) || !pendingCaptureWakes.has(requestId)) return;
    if (isRetry) deferredWakeRetrySchedules += 1;
    else {
      deferredWakeSchedules += 1;
      exports.fission_web_test_deferred_wake_scheduled();
    }
    const attempt = captureWakeRetryAttempts.get(requestId) || 0;
    const delay = isRetry ? captureWakeRetryDelay(attempt) : 0;
    if (isRetry) captureWakeRetryAttempts.set(requestId, attempt + 1);
    const timer = globalThis.setTimeout(() => {
      captureWakeTimers.delete(requestId);
      if (!pendingCaptureWakes.has(requestId)) return;
      deferredWakeCalls += 1;
      if (isRetry) deferredWakeRetryCalls += 1;
      try {
        // A repeated wake is safe while this request remains in the Rust
        // capture queue: the event-loop drain removes it before dispatching
        // the render capture, and the request response remains single-shot.
        const sent = (exports.fission_web_test_wake_event_loop() >>> 0) === 1;
        deferredWakeExportReturns += 1;
        lastDeferredWakeSendResult = sent;
        if (sent) deferredWakeSendSuccesses += 1;
      } catch (error) {
        deferredWakeFailures += 1;
        throw error;
      }
    }, delay);
    captureWakeTimers.set(requestId, timer);
  };

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
      // returned. If Winit accepts a proxy event but misses its callback, a
      // later null poll schedules another wake while this capture remains in
      // Rust's queue. Poll never invokes Wasm recursively.
      pendingCaptureWakes.add(requestId);
      try {
        sendDeferredWake(requestId, false);
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
      const json = decoder.decode(new Uint8Array(memory.buffer, address, length));
      if (pendingCaptureWakes.has(requestId)) {
        if (json !== "null") {
          clearCaptureWake(requestId);
        } else sendDeferredWake(requestId, true);
      }
      return json;
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
        captureRequestsPending: typeof exports.fission_web_test_capture_requests_pending === "function"
          ? (exports.fission_web_test_capture_requests_pending() >>> 0) === 1
          : null,
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
          deferredWakeRetrySchedules,
          deferredWakeRetryCalls,
        },
      };
    },
  };
}
