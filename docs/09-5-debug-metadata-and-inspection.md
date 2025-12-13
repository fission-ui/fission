# 9.5 Debug Metadata and Inspection

This section defines the debug metadata model attached to display lists and paint operations, and how that metadata is used for inspection, diagnostics, and tooling.
Debug metadata is strictly non-semantic: it must never affect layout, painting, or behavior.

Debuggability is a first-class concern, but correctness is never conditional on debug data.

---

## 9.5.1 Goals of Debug Metadata

Debug metadata exists to:

- explain *why* a draw operation exists,
- correlate visuals with Core IR and NodeIds,
- support tooling, inspection, and diagnostics,
- enable rich failure messages in tests.

Debug metadata must be optional and zero-cost when disabled.

---

## 9.5.2 Non-Semantic Guarantee

Rules:
- debug metadata does not participate in equality,
- debug metadata does not affect determinism,
- absence of metadata must not change output.

All production behavior must be identical with metadata stripped.

---

## 9.5.3 Metadata Attachment Points

Metadata may be attached to:

- display list (global metadata),
- individual display ops,
- paint spans (via PaintMap),
- compilation decisions (optional traces).

Attachment points are explicit and versioned.

---

## 9.5.4 Core Metadata Fields

Typical metadata fields include:

- originating NodeId,
- originating Core IR op id,
- source widget/type name,
- authoring provenance (file/module),
- semantic role (if applicable).

Fields are structured, not free-form strings.

---

## 9.5.5 Provenance Chains

Metadata supports provenance chains.

Example:
- Widget → Authoring Node → Core IR Ops → Display Ops

Provenance chains enable:
- precise error attribution,
- cross-layer inspection,
- LLM-assisted explanations.

Chains are acyclic and deterministic.

---

## 9.5.6 Inspection APIs

The framework exposes inspection APIs to:

- query metadata for a display op,
- map screen coordinates to NodeIds,
- retrieve provenance for a paint span,
- visualize paint order and bounds.

Inspection APIs are read-only.

---

## 9.5.7 Debug Views and Overlays

Tooling may use metadata to render debug views:

- bounding box overlays,
- paint order indices,
- clip regions,
- NodeId labels.

Debug views are layered atop rendering and never feed back into the pipeline.

---

## 9.5.8 Testing and Failure Diagnostics

Debug metadata improves test diagnostics.

Examples:
- “expected button background to be drawn before icon”
- “node X produced no paint ops”

Tests must not assert on debug metadata directly, but metadata is included in failure output.

---

## 9.5.9 Performance Considerations

Debug metadata is designed to be:

- lazily attached,
- optionally compiled out,
- stored in compact representations.

When disabled, there is no runtime or memory overhead beyond conditional branches.

---

## 9.5.10 Security and Privacy

Debug metadata may include source-level information.

Rules:
- metadata is disabled by default in production,
- tooling must not expose metadata unintentionally,
- redaction is supported for sensitive fields.

---

## 9.5.11 Versioning and Stability

Debug metadata schemas are versioned.

Rules:
- adding fields is backward-compatible,
- removing fields requires version bumps,
- tooling must handle unknown fields gracefully.

Metadata evolution must not break older tools.

---

## 9.5.12 Summary

Debug metadata and inspection facilities:

- make rendering explainable,
- connect pixels back to structure and intent,
- dramatically improve diagnostics and tooling,
- preserve determinism and performance.

Debuggability is engineered, not bolted on.

---
