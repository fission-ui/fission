# 5.7 Provenance and Debug Metadata

This section defines how provenance and debug metadata are attached throughout lowering and canonicalization.
Provenance enables precise diagnostics, explainable diffs, and powerful tooling—without affecting semantics or performance when disabled.

Debug metadata is optional, structured, and strictly non-semantic.

---

## 5.7.1 Goals of Provenance Tracking

Provenance exists to:

- explain *why* a Core IR node exists,
- trace Core nodes back to authoring constructs,
- support precise error reporting,
- enable rich inspection and tooling,
- aid LLM-assisted reasoning and refactoring.

Provenance never changes behavior.

---

## 5.7.2 Non-Semantic Guarantee

Debug metadata is explicitly non-semantic.

Rules:
- it must not affect layout, paint, or interaction,
- it must not participate in identity derivation,
- it must not influence canonicalization decisions.

Two Core IR trees that differ only in debug metadata are semantically identical.

---

## 5.7.3 Provenance Model

Each Core IR node may carry optional provenance:

```rust
pub struct Provenance {
    pub source: SourceRef,
    pub widget_path: Vec<WidgetRef>,
    pub lowering_stage: LoweringStage,
}
```

Provenance is immutable once attached.

---

## 5.7.4 Source References

### `SourceRef`

Represents where a node originated.

Examples:
- file + line + column,
- generated code span,
- synthetic location for derived nodes.

Source references:
- are optional,
- may be coarse-grained,
- are best-effort, not guaranteed.

---

## 5.7.5 Widget Path Tracking

### `WidgetRef`

Tracks the authoring-level path that led to a Core node.

Properties:
- ordered list from root widget to leaf,
- includes custom widget boundaries,
- survives desugaring and expansion.

This enables tools to answer:
“Which widget produced this Core node?”

---

## 5.7.6 Lowering Stage Attribution

### `LoweringStage`

Indicates when a node was introduced:

- authoring capture,
- widget desugaring,
- structural expansion,
- canonicalization.

This helps distinguish:
- user-authored structure,
- framework-generated structure.

---

## 5.7.7 Debug Metadata Attachment Rules

Metadata may be attached:

- during desugaring,
- during structural expansion,
- during canonicalization (for retained nodes only).

Metadata must be:
- propagated when nodes are copied,
- discarded when nodes are elided.

---

## 5.7.8 Interaction With Canonicalization

Canonicalization respects provenance:

- merged nodes combine provenance conservatively,
- elided nodes drop metadata,
- reordered nodes retain metadata unchanged.

Canonicalization must not fabricate provenance.

---

## 5.7.9 Error Reporting and Diagnostics

When errors occur, provenance enables:

- precise source locations,
- widget-level explanations,
- actionable error messages.

Example:
> “Invalid layout constraint introduced by `Row` at src/ui/counter.rs:42”

This dramatically improves developer experience.

---

## 5.7.10 Tooling and Visualization

Provenance enables advanced tooling:

- Core IR viewers with source mapping,
- diff tools that explain changes,
- test failures annotated with widget paths,
- LLM tools that reason about intent vs outcome.

All without impacting runtime behavior.

---

## 5.7.11 Zero-Cost When Disabled

Provenance is optional.

When disabled:
- no metadata is allocated,
- no extra memory is consumed,
- no performance penalty is incurred.

This is enforced via feature flags and type erasure.

---

## 5.7.12 Summary

Provenance and debug metadata provide:

- explainability without semantic impact,
- powerful diagnostics and tooling,
- improved test and diff clarity,
- optional, zero-cost instrumentation.

They are essential for developer productivity while preserving the framework’s core guarantees.

---
