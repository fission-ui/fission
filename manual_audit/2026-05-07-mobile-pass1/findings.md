# Mobile audit: 2026-05-07 pass 1

## Scope

Targets audited on the current branch head:

- iOS simulator via `examples/mobile-smoke/platforms/ios/run-sim.sh`
- Android emulator via `examples/mobile-smoke/platforms/android/run-emulator.sh`

Artifacts live under:

- `manual_audit/2026-05-07-mobile-pass1/ios/`
- `manual_audit/2026-05-07-mobile-pass1/android_emulator.log`

## Findings

### 1. iOS simulator renders into only a small top-left region

Severity: high

The iOS simulator path is alive and interactive, but the app only paints into a small region in the top-left of the device surface. The rest of the simulator is blank white.

Observed behavior:

- text and button content are constrained to a narrow area around `272px` wide
- the button label breaks into two lines (`Increme` / `nt`)
- most of the `960x1380` simulator surface is blank
- tap interaction works (`Taps: 0` -> `Taps: 1`), so this is not a dead app; it is a layout / viewport / render-target mismatch

Evidence:

- `manual_audit/2026-05-07-mobile-pass1/ios/01_initial.png`
- `manual_audit/2026-05-07-mobile-pass1/ios/02_after_tap.png`
- `manual_audit/2026-05-07-mobile-pass1/ios/03_full_simulator.png`
- `manual_audit/2026-05-07-mobile-pass1/ios/01_text.json`
- `manual_audit/2026-05-07-mobile-pass1/ios/02_text_after_tap.json`

Likely cause:

- the simulator renderer fallback is working, but the viewport/env/layout width is not being synchronized to the real simulator surface size
- alternatively the fallback renderer is drawing to a stale or incorrectly scaled logical surface while the outer target texture is full device size

### 2. Android emulator path is unstable after launch

Severity: high

The Android path does not stay healthy long enough to complete a basic manual audit round-trip.

Observed behavior:

- `run-emulator.sh` booted the emulator, packaged, installed, and launched the app
- health briefly responded on the forwarded host port
- the audit request sequence then stalled and timed out
- shortly afterwards `adb devices` reported no connected devices/emulators
- the emulator process did not remain available for a normal screenshot/test-control pass

Evidence:

- `manual_audit/2026-05-07-mobile-pass1/android_emulator.log`

Notable log signal near the failure:

- `SPIR-V ERROR: 0:0 Invalid source language operand: 10`

This needs to be treated as a runtime stability blocker, not just a missing test hook.

### 3. Android visible-run behavior is not trustworthy yet

Severity: medium

The script claimed a visible emulator launch, but the resulting path was not durable enough to inspect by hand and did not remain connected over `adb`.

Even if the root cause is the graphics/runtime crash above, the current developer story is still poor because:

- the run script can print a success path while the target is already unstable
- a third-party developer would read this as "the emulator run works" when it is not yet reliable

## Next fix order

1. fix the iOS simulator viewport/render-target mismatch so the app fills the device surface correctly
2. stabilize the Android emulator graphics/runtime path until `adb`, test control, and screenshots survive launch
3. rerun the same audit on both targets and replace this findings file with the next pass
