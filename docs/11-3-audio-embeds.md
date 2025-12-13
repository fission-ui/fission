# 11.3 Audio Embeds

This section defines **audio embeds**.
Audio is treated as a semantic, stateful media element with no visual geometry, integrated into the same deterministic action, state, and testing model as all other systems.

Audio answers *what is played* and *when*, never *how pixels are drawn*.

---

## 11.3.1 Design Goals

Audio embeds must:

- be deterministic and replayable,
- integrate with the action and reducer model,
- expose explicit playback state,
- participate in accessibility,
- be testable headlessly without sound hardware.

Audio playback must not influence layout or painting.

---

## 11.3.2 Audio as a Core Embed

An audio embed is represented by:

- a Core IR embed node,
- a referenced audio resource,
- explicit playback state,
- declared semantics and actions.

Audio nodes do not emit paint ops.

---

## 11.3.3 Layout Semantics

Audio embeds have no visual layout.

Rules:
- they occupy zero size by default,
- they do not affect layout constraints,
- they may still participate in focus order and semantics.

If a UI requires visible controls, those are separate visual nodes.

---

## 11.3.4 Playback State Model

Audio playback state includes:

- current playback position,
- duration (if known),
- playing / paused / stopped flags,
- playback rate,
- buffering / readiness state.

All state is explicit and serializable.

---

## 11.3.5 Actions and Reducers

Audio playback is controlled via actions:

- `Play`
- `Pause`
- `Stop`
- `Seek { time }`
- `SetRate { rate }`

Reducers update playback state deterministically.
IO and decoding occur in isolated subsystems.

---

## 11.3.6 Time Ownership and Progression

Audio time advances only via explicit ticks.

Rules:
- no wall-clock time is consulted,
- time deltas are explicit inputs,
- playback progression is replayable.

Audio behavior can be simulated exactly in tests.

---

## 11.3.7 Painting and Rendering Interaction

Audio does not participate in painting.

Rules:
- no display list ops are emitted,
- audio state does not affect paint order,
- visualization (e.g. waveforms) is a separate concern.

Rendering systems may observe audio state but not depend on it.

---

## 11.3.8 Accessibility Semantics

Audio embeds expose accessibility semantics:

- role: audio / media,
- state: playing, paused, stopped,
- actions: play, pause, seek.

Screen readers interact via the same action system.

---

## 11.3.9 Headless Testing of Audio

Audio is fully testable headlessly.

Strategies:
- stub audio resources,
- simulate time advancement,
- assert state transitions.

Example:

```rust
dispatch(Play);
dispatch(Tick { dt: 1000 });
assert_eq!(find("audio").position(), 1000);
```

No audio output device is required.

---

## 11.3.10 Error Handling and Fallbacks

Audio errors are explicit states.

Rules:
- load or decode failures do not crash the app,
- errors are observable and testable,
- fallback behavior is deterministic.

---

## 11.3.11 Platform Integration

Platform audio backends may be used underneath.

Rules:
- platform timing is ignored,
- decoded samples are scheduled deterministically,
- backend differences must not affect state evolution.

Backends are replaceable without semantic changes.

---

## 11.3.12 Performance Considerations

Performance strategies include:

- buffered decoding,
- streaming where appropriate,
- shared audio pipelines.

Performance optimizations must not alter observable state.

---

## 11.3.13 Summary

Audio embeds are deterministic because:

- playback state is explicit,
- time is owned by the runtime,
- actions drive all behavior,
- layout and paint are unaffected.

Audio is part of the state machine, not a side effect.

---
