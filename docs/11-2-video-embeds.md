# 11.2 Video Embeds

This section defines the model for **video embeds**.
Video is treated as a visual surface with explicit state and lifecycle, separated from layout and painting semantics to preserve determinism and testability.

Video playback is stateful; video layout and paint are deterministic.

---

## 11.2.1 Separation of Concerns

Video embeds are split into two orthogonal concerns:

1. **Presentation**
   - layout size and aspect ratio,
   - paint bounds and clipping,
   - transforms and opacity.

2. **Playback**
   - current time and frame,
   - play / pause / seek state,
   - buffering and readiness.

Only presentation participates directly in layout and painting.

---

## 11.2.2 Video as a Core Embed

A video embed is represented by:

- a Core IR embed node,
- a referenced video resource,
- explicit sizing and aspect rules,
- explicit playback state reference,
- declared interaction and semantics.

There is no implicit platform video widget.

---

## 11.2.3 Intrinsic Size and Aspect Ratio

Video intrinsic size is defined as:

- the natural pixel dimensions of the video track,
- scaled by an explicit logical scale factor.

Rules:
- intrinsic size is metadata-driven,
- aspect ratio is explicit and preserved by default,
- layout never depends on playback state.

---

## 11.2.4 Layout Participation

Video participates in layout like an image:

- intrinsic size may be used,
- constraints may override size,
- aspect handling is explicit,
- clipping and transforms apply normally.

Video never causes layout reflow during playback.

---

## 11.2.5 Painting Video Frames

Painting emits a **surface draw op**.

Rules:
- the current decoded frame is treated as a texture,
- geometry comes from the layout snapshot,
- paint bounds are conservative,
- clipping and transforms apply deterministically.

If no frame is available, a placeholder is painted.

---

## 11.2.6 Playback State as Explicit Data

Playback state includes:

- playback position,
- rate,
- paused / playing flags,
- buffering / ready state.

Playback state is owned by application/runtime state and updated via actions.

---

## 11.2.7 Actions and Reducers

Video playback is controlled via actions:

- `Play`
- `Pause`
- `Seek { time }`
- `SetRate { rate }`

Reducers update playback state deterministically.
Decoding and IO are side-effectful but isolated.

---

## 11.2.8 Time and Determinism

Video time advances only via explicit ticks.

Rules:
- wall-clock time is never consulted,
- time deltas are explicit,
- playback progression is replayable.

Tests can advance video deterministically.

---

## 11.2.9 Headless Testing of Video

Headless tests do not require real video playback.

Strategies include:
- deterministic frame providers,
- fixed-frame test assets,
- placeholder rendering.

Tests may assert:
- layout geometry,
- paint op presence,
- state transitions.

---

## 11.2.10 Accessibility Semantics

Video embeds expose semantics:

- role: video / media,
- actions: play, pause, seek,
- state: playing, paused, position.

Accessibility invokes the same actions as input and tests.

---

## 11.2.11 Error Handling and Fallbacks

Video errors are explicit states.

Rules:
- decode or load failures do not crash layout,
- fallback visuals are deterministic,
- errors are observable and testable.

---

## 11.2.12 Platform Integration

Platform decoders may be used underneath.

Rules:
- decoder behavior is sandboxed,
- outputs are normalized into deterministic frames,
- platform variance must not affect layout or ordering.

Renderer backends consume normalized surfaces only.

---

## 11.2.13 Performance Considerations

Performance strategies include:

- frame caching,
- surface reuse,
- asynchronous decode pipelines.

Optimizations must not alter observable behavior.

---

## 11.2.14 Summary

Video embeds are deterministic because:

- layout and paint are independent of playback,
- playback state is explicit and action-driven,
- time advancement is controlled,
- testing does not rely on real-time behavior.

Video is treated as stateful media, not magical UI.

---
