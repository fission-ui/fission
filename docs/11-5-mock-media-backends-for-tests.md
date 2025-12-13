# 11.5 Mock Media Backends for Tests

This section defines **mock and fake media backends** used for deterministic testing of images, video, and audio.
Mock backends ensure that media-related behavior is fully testable without relying on real decoders, hardware, or timing sources.

Tests must never depend on platform media stacks.

---

## 11.5.1 Purpose of Mock Media Backends

Mock media backends exist to:

- eliminate platform and hardware dependencies,
- provide deterministic media behavior,
- enable headless CI execution,
- support precise assertions over media state,
- allow fault injection and edge-case testing.

Mocks are not approximations; they are reference implementations.

---

## 11.5.2 Mock vs Fake vs Real Backends

The framework distinguishes:

- **Real backends**: platform-integrated implementations (production),
- **Fake backends**: deterministic implementations with simplified behavior,
- **Mock backends**: programmable test doubles with scripted behavior.

All backends conform to the same backend interface.

---

## 11.5.3 Backend Interface Contract

Media backends implement a strict interface:

- resource resolution by identifier,
- lifecycle callbacks (activate, deactivate, release),
- frame or sample production,
- error reporting,
- deterministic behavior under explicit time control.

Tests rely on the interface contract, not concrete implementations.

---

## 11.5.4 Mock Image Backends

Mock image backends provide:

- fixed pixel buffers,
- explicit intrinsic sizes,
- deterministic decode outcomes,
- configurable failure modes.

Example uses:
- validating layout and paint behavior,
- testing error fallbacks,
- golden raster tests.

---

## 11.5.5 Mock Video Backends

Mock video backends simulate video playback by:

- providing deterministic frame sequences,
- advancing frames only on explicit ticks,
- exposing fixed frame dimensions and rates,
- allowing scripted stalls or errors.

Playback never depends on wall-clock time.

---

## 11.5.6 Mock Audio Backends

Mock audio backends simulate audio playback by:

- advancing playback position deterministically,
- ignoring actual audio output,
- exposing explicit buffering and error states.

This enables precise testing of playback logic and reducers.

---

## 11.5.7 Time Control in Tests

All mock backends depend on explicit time control.

Rules:
- time advances only via `Tick` actions,
- backends must not spawn timers,
- time-based behavior is replayable.

Tests fully control temporal progression.

---

## 11.5.8 Scripted Behavior and Fault Injection

Mocks may be scripted to:

- fail on load,
- stall decoding,
- return corrupted frames,
- drop samples.

Fault injection is deterministic and reproducible.

---

## 11.5.9 Snapshot Integration

Mock backend state is reflected in snapshots.

Snapshots may expose:
- current frame index,
- playback position,
- error states.

This enables snapshot-based regression testing.

---

## 11.5.10 Golden Testing With Mocks

Mock backends are ideal for golden tests.

Benefits:
- stable pixel output,
- no external dependencies,
- reproducible failures.

Golden tests validate the full pipeline deterministically.

---

## 11.5.11 Platform Parity Guarantees

Mocks define the reference behavior.

Rules:
- real backends must match mock-observable semantics,
- deviations are considered bugs,
- conformance tests compare real vs mock behavior.

Mocks serve as executable specifications.

---

## 11.5.12 Performance Considerations

Mock backends are optimized for test speed.

Rules:
- minimal allocations,
- predictable memory usage,
- fast setup and teardown.

Performance realism is not a goal.

---

## 11.5.13 Summary

Mock media backends:

- make media behavior deterministic and testable,
- decouple tests from platforms and hardware,
- support fault injection and regression testing,
- define the semantic contract for real backends.

If media cannot be mocked, it is not deterministic.

---
