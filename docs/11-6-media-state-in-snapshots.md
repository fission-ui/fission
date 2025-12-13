# 11.6 Media State in Snapshots

This section defines how **media-related state** (images, video, audio, and other embeds) is represented in snapshots.
Media state must be fully observable, serializable, and replayable to support determinism, testing, tooling, and debugging.

If media state is not present in the snapshot, it must be treated as nonexistent.

---

## 11.6.1 Goals of Media State Snapshots

Media state snapshots exist to:

- expose current media-related state deterministically,
- decouple media behavior from live backends,
- support headless testing and replay,
- enable precise inspection and debugging,
- allow snapshot diffing for regressions.

Snapshots are the authoritative record of media state.

---

## 11.6.2 Snapshot Layers Involved

Media state appears across multiple snapshot layers:

- **Layout Snapshot**: geometry and embed placement,
- **Paint / Display Snapshot**: visual representation (if any),
- **Runtime State Snapshot**: playback and lifecycle state.

Each layer has a clear, non-overlapping responsibility.

---

## 11.6.3 Common Media State Fields

Across media types, snapshots may expose:

- resource identifier and version,
- lifecycle phase (declared, active, released),
- error state (if any),
- backend identifier (mock, real, type),
- last update tick.

These fields are explicit and versioned.

---

## 11.6.4 Image State in Snapshots

Image snapshot data includes:

- intrinsic size (logical and pixel),
- decode status (pending, ready, failed),
- fallback status (if applicable),
- resolved sampling mode.

Image state does not change after activation, except on error.

---

## 11.6.5 Video State in Snapshots

Video snapshot data includes:

- current playback position,
- duration (if known),
- playing / paused state,
- current frame index or timestamp,
- readiness / buffering state,
- active physics or rate configuration.

All fields are derived from explicit reducer state.

---

## 11.6.6 Audio State in Snapshots

Audio snapshot data includes:

- current playback position,
- duration (if known),
- playing / paused / stopped state,
- playback rate,
- buffering or error state.

Audio state is observable without audio output.

---

## 11.6.7 External / Native Embed State

External embed snapshot data may include:

- surface handle identifier (opaque),
- activation status,
- focus participation,
- error or unavailable state.

Platform-specific details are abstracted behind stable identifiers.

---

## 11.6.8 Determinism Guarantees

Media state snapshots are deterministic because:

- state evolves only via actions and reducers,
- time advancement is explicit,
- backend behavior is normalized,
- snapshots capture all observable state.

Identical traces produce identical snapshots.

---

## 11.6.9 Snapshot Diffing and Regression Detection

Media-aware snapshot diffs can detect:

- unintended playback changes,
- incorrect lifecycle transitions,
- resource resolution regressions,
- platform inconsistencies.

Diffs operate on structured data, not logs.

---

## 11.6.10 Serialization and Persistence

Media state snapshots are serializable.

Uses include:
- golden tests,
- offline debugging,
- bug reproduction,
- trace replay across machines.

Serialization formats are versioned and stable.

---

## 11.6.11 Inspection APIs

Inspection APIs may query:

- media state by NodeId,
- lifecycle phase transitions,
- playback position at a given tick,
- error provenance.

Inspection is read-only and side-effect free.

---

## 11.6.12 Interaction With Rendering

Rendering consumes only visual aspects of media state.

Rules:
- playback state does not affect layout,
- paint output is derived from snapshot state,
- rendering never mutates media state.

Snapshots are a one-way contract.

---

## 11.6.13 Error States and Fallbacks

Error states are explicit in snapshots.

Rules:
- errors persist until cleared by action,
- fallback visuals are reflected in paint snapshots,
- errors are testable and replayable.

No silent recovery is allowed.

---

## 11.6.14 Summary

Media state in snapshots:

- makes media behavior observable and testable,
- decouples UI logic from live backends,
- enables deterministic replay and debugging,
- supports regression detection across platforms.

Snapshots turn media from side effects into data.

---
