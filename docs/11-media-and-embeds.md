# 11. Media and Embeds

This section defines how **media and embedded content** (images, video, audio, web views, native surfaces) are modeled in the framework.
Media is treated as explicit data and lifecycle, not opaque widgets, preserving determinism, testability, and portability.

Media answers two questions only:
- *what is embedded?*
- *how it participates in layout, painting, input, and testing?*

---

## 11.1 Design Goals

Media and embeds must:

- integrate with Core IR without special cases,
- participate deterministically in layout and painting,
- expose explicit lifecycle and state,
- be testable headlessly,
- degrade gracefully across platforms.

Playback and IO are orthogonal concerns.

---

## 11.2 Media as Core-Primitives + Resources

Media nodes consist of:

- a Core IR embed op (structural),
- a referenced resource (image, stream, surface),
- explicit sizing and aspect rules,
- explicit interaction and semantics.

Resources are identified, versioned, and cached deterministically.

---

## 11.3 Image Embeds

Images are the simplest embed.

Properties:
- intrinsic size (from resource metadata),
- explicit fit mode (contain, cover, fill),
- deterministic decoding and color handling.

Image painting produces standard draw-image ops in the display list.

---

## 11.4 Video Embeds

Video embeds separate **presentation** from **playback**.

Core responsibilities:
- reserve layout space,
- expose current frame as a texture/surface,
- integrate with clipping and transforms.

Playback state (play, pause, seek) is explicit state driven by actions.

---

## 11.5 Audio Embeds

Audio embeds are non-visual but semantic.

Properties:
- no paint output,
- explicit duration and state,
- actions for play/pause/seek,
- accessibility semantics (media controls).

Audio participates in state and action tracing but not layout geometry.

---

## 11.6 Native and External Embeds

External surfaces (web views, platform widgets) are modeled as embeds.

Rules:
- geometry and clipping are owned by the framework,
- rendering is delegated via a surface handle,
- hit testing and focus routing are explicit.

External embeds are sandboxed and observable.

---

## 11.7 Layout Semantics for Media

Media participates in layout like any other node.

Rules:
- intrinsic sizing is explicit and cached,
- aspect ratio constraints are deterministic,
- media never triggers layout reflow implicitly.

Layout does not depend on playback state.

---

## 11.8 Painting and Display Lists

Media painting emits explicit ops:

- image draw ops,
- surface draw ops (for video/external),
- placeholder ops when unavailable.

Paint bounds are conservative and deterministic.

---

## 11.9 Input, Focus, and Semantics

Media nodes expose semantics:

- roles (image, video, media control),
- actions (play, pause, activate),
- focus behavior.

All interactions route through the action system.

---

## 11.10 Headless Testing of Media

Headless tests do not require real playback.

Strategies:
- stub media resources,
- deterministic frame providers,
- placeholder rendering.

Tests may assert:
- layout and bounds,
- emitted paint ops,
- action wiring and state changes.

---

## 11.11 Resource Management and Caching

Resources are:

- loaded deterministically,
- reference-counted or pooled,
- invalidated explicitly.

IO timing does not affect layout or paint determinism.

---

## 11.12 Error Handling and Fallbacks

Media errors are explicit states.

Rules:
- failures do not crash layout,
- fallback visuals are deterministic,
- errors are observable and testable.

---

## 11.13 Security and Sandboxing

Embeds may execute untrusted content.

Rules:
- strict isolation boundaries,
- no implicit data access,
- explicit permission surfaces.

Security is a platform concern, semantics are Core-owned.

---

## 11.14 Future Extensions

The model supports future media types:

- 3D scenes,
- live streams,
- collaborative embeds,
- LLM-driven media controls.

All extensions must preserve explicit state and determinism.

---

## 11.15 Summary

Media and embeds:

- are explicit Core participants,
- integrate cleanly with layout, paint, and input,
- remain deterministic and testable,
- support rich content without sacrificing correctness.

Embedding content should never embed ambiguity.

---
