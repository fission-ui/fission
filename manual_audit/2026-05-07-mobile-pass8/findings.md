# Mobile audit: 2026-05-07 pass 8

## Scope

Targets audited on the current branch head:

- iOS simulator via `examples/mobile-smoke/platforms/ios/run-sim.sh`
- Android emulator via `examples/mobile-smoke/platforms/android/run-emulator.sh`

Artifacts live under:

- `manual_audit/2026-05-07-mobile-pass8/ios/`
- `manual_audit/2026-05-07-mobile-pass8/android/`

## Result

No blocking smoke-level UI defects remain for `examples/mobile-smoke` on either audited mobile target.

The branch now passes the intended round-trip on both targets:

- launch/install succeeds
- the app renders visible pixels into the full device surface
- test control responds over `FISSION_TEST_CONTROL_PORT`
- semantic text queries return the expected content
- tap interaction updates visible state
- screenshots return real pixels instead of blank/black frames

## Evidence

### iOS simulator

- visible app-surface capture:
  - `manual_audit/2026-05-07-mobile-pass8/ios/03_capture.png`
- full simulator screenshot:
  - `manual_audit/2026-05-07-mobile-pass8/ios/04_full_simulator.png`
- semantic responses:
  - `manual_audit/2026-05-07-mobile-pass8/ios/00_health.json`
  - `manual_audit/2026-05-07-mobile-pass8/ios/01_text.json`
  - `manual_audit/2026-05-07-mobile-pass8/ios/02_tree.json`
  - `manual_audit/2026-05-07-mobile-pass8/ios/03_capture.json`

### Android emulator

- app-surface capture before interaction:
  - `manual_audit/2026-05-07-mobile-pass8/android/05_capture.png`
- app-surface capture after tap:
  - `manual_audit/2026-05-07-mobile-pass8/android/06_capture_after_tap.png`
- full device screenshot after tap:
  - `manual_audit/2026-05-07-mobile-pass8/android/07_full_device.png`
- semantic responses:
  - `manual_audit/2026-05-07-mobile-pass8/android/00_health.json`
  - `manual_audit/2026-05-07-mobile-pass8/android/01_health_after_fix.json`
  - `manual_audit/2026-05-07-mobile-pass8/android/02_text_after_fix.json`

## Remaining platform caveats

These are explicit follow-up items, not current smoke blockers:

### 1. iOS simulator uses the software renderer fallback

Severity: medium

CoreSimulator still does not expose `DownlevelFlags(INDIRECT_EXECUTION)`, so the shell falls back to the shared software renderer on the simulator. The path is now correct and usable, but it is not representative of hardware iOS GPU performance.

### 2. Android emulator defaults to GLES for stability

Severity: medium

The shell now forces `WGPU_BACKEND=gl` on Android when `WGPU_BACKEND` is unset. This keeps the emulator off the unstable Vulkan/SwiftShader path that was crashing during surface configuration.

That is the right default for emulator reproducibility, but real-device Vulkan behavior still needs separate audit coverage.

### 3. Android test-control wakeup still uses an emulator-specific polling fallback

Severity: low

When `FISSION_TEST_CONTROL_PORT` is enabled on Android, the shell keeps the event loop polling so queued semantic test-control commands wake and drain reliably inside the emulator.

This makes the automation path work today, but it should eventually be replaced with a lower-overhead native wake mechanism.

## Next work

1. keep the current mobile smoke path stable and turn it into regression coverage
2. audit Android on a physical device, including explicit `WGPU_BACKEND=vulkan` coverage
3. audit hardware iOS once the device shell/signing path is in place
4. replace the Android emulator polling workaround with a proper wake path when available
