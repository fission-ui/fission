# 12.3 Starting, Updating, and Ending Animations

This section specifies the **exact mechanics** for starting, progressing, and ending animations.
All animation transitions are explicit, action-driven, and deterministic.

Animations never start, advance, or end implicitly.

---

## 12.3.1 Animation Start Semantics

Animations are started exclusively via actions.

Example actions:
- `StartAnimation`
- `ReplaceAnimation`
- `RestartAnimation`

Start rules:
- the start time is sampled from the owned clock,
- the start value is resolved explicitly,
- parameters are validated eagerly.

Starting an animation is a pure state transition.

---

## 12.3.2 Resolving Start Values

Start values are resolved deterministically.

Rules:
- explicit `from` value takes precedence,
- otherwise, the current resolved state value is sampled,
- sampling occurs atomically at start time.

There is no race with rendering or layout.

---

## 12.3.3 Animation Registration

On start, the animation is registered with the runtime service.

Registration includes:
- unique animation ID,
- target state path,
- timing parameters,
- easing function,
- lifecycle state set to `Active`.

Registration order is deterministic.

---

## 12.3.4 Updating Animations (Progression)

Animations progress only when time advances.

Rules:
- progression occurs during reducer evaluation on `Tick`,
- progress = clamp((now - start_time) / duration),
- easing is applied deterministically.

No animation progresses without a tick.

---

## 12.3.5 Partial Progress and Sampling

At any tick, the resolved value is:

```
value = lerp(from, to, ease(progress))
```

Rules:
- numeric precision is fixed,
- rounding rules are explicit,
- sampling is stable across platforms.

Intermediate values are fully inspectable.

---

## 12.3.6 Ending Animations

An animation ends when:

- progress reaches 1.0, or
- it is cancelled explicitly.

On completion:
- final value is written once,
- lifecycle transitions to `Completed`,
- optional completion actions may be emitted.

Completion is deterministic and observable.

---

## 12.3.7 Cancellation Semantics

Cancellation is explicit.

Rules:
- cancelling an animation freezes its contribution,
- optional snap-to-end is configurable,
- cancelled animations are removed deterministically.

There is no implicit cancellation.

---

## 12.3.8 Replacement Semantics

Replacing an animation is an explicit operation.

Rules:
- replacement samples the current value,
- new parameters are applied deterministically,
- ownership of the target field is transferred.

Replacement avoids discontinuities unless explicitly requested.

---

## 12.3.9 Concurrent Animations

Multiple animations may target:

- different state fields freely,
- the same field only with explicit composition rules.

Default conflict policy is deterministic and enforced.

---

## 12.3.10 Interaction With Layout and Painting

Animation updates do not directly trigger rendering.

Rules:
- state updates mark layout/paint as dirty,
- recomputation occurs in the normal pipeline,
- animation does not bypass diffing or caching.

The pipeline remains uniform.

---

## 12.3.11 Snapshot Representation

Snapshots capture animation lifecycle state:

- start time,
- progress,
- resolved value,
- lifecycle phase.

Snapshots enable replay and debugging.

---

## 12.3.12 Error Conditions

Animation lifecycle errors include:

- zero or negative duration,
- invalid target paths,
- duplicate animation IDs,
- conflicting ownership without policy.

Errors are detected eagerly and fail deterministically.

---

## 12.3.13 Summary

Starting, updating, and ending animations are:

- explicit and action-driven,
- tied to owned time,
- reducer-evaluated,
- observable and testable.

Animations evolve state in a controlled, predictable manner.

---
