# Manual audit and regression loop

This is the required iteration model for UI/platform work in Fission.

## Why
Automated tests have not been sufficient to catch:
- overlay and portal corruption
- resize/compositor invalidation bugs
- scroll routing failures
- text style inheritance mistakes
- menu/popup semantic mismatches
- stale layer retention after close/apply transitions

The examples expose these failures, but the fixes belong mainly in the platform.

## Required loop
1. Make the code changes.
2. Run the full automated test suite for affected crates, then the full workspace if the change is broad.
3. Run a full manual audit pass across all example apps.
4. Capture screenshots after each meaningful action.
5. Inspect the screenshots manually and record findings.
6. Convert each finding into the highest-leverage regression test possible.
   - Prefer core/runtime/layout/render/widget tests over example-only tests.
   - Keep example-level tests only when the behavior truly depends on example composition.
7. Confirm the new tests fail.
8. Implement fixes until the new and existing tests pass.
9. Run the manual audit pass again under a new `passN` directory.
10. Do not declare the change complete until both the automated tests and the repeated audit pass are clean.

## Audit scope
Every pass must cover every app in `examples/`.

Current set:
- `animation-gallery`
- `chart-gallery`
- `counter`
- `editor`
- `icons_gallery`
- `inbox`
- `text-lab`
- `widget-gallery`

## Artifact rules
- Store each pass under `manual_audit/YYYY-MM-DD-passN/`.
- Store screenshots in per-app subdirectories.
- Keep a pass-level `findings.md`.
- Reference screenshots directly from findings.

## Test translation rules
Map findings downward toward the platform:
- viewport/surface mismatch -> shell desktop tests
- resize invalidation/compositor corruption -> shell desktop + rendering tests
- popup/menu/portal geometry -> widgets + core layout/interaction tests
- scroll routing failures -> runtime/input/layout tests
- text/input command leakage -> core text/input tests
- theme/default text color failures -> widget/theme tests
- stale layer retention -> compositor/runtime invalidation tests

## Exit criteria
A change is complete only when:
- all relevant automated tests pass
- the full example audit has been rerun
- the rerun audit does not show visual or functional regressions
- any newly discovered issue has either been fixed or converted into a tracked failing regression test
