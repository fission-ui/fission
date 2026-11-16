# Web Canvas2D fallback spends seconds rendering modal backdrop blur

Status: the Web Canvas2D fallback again preserves declared backdrop filters.
Silently omitting blur changed the application's appearance and was not a
correctness-preserving optimisation. The historical measurements below identify
an unresolved software-rasterisation cost; they do not qualify the current
renderer or establish that its modal response time is interactive.

## Reproduction and measurements

An application measured the Fission Web target at 1280×900 with `web-perf`
enabled and `canvas2d-software` selected. Opening a modal took 3,987.68 ms
for the frame: 3,957.025 ms in software rasterization and 0.975 ms in Canvas
upload. The software raster stage, rather than canvas presentation, dominates
the delay.

The modal scrim applies a 4 px backdrop blur. Fission's Web Canvas2D presenter
renders the retained scene through `CpuRenderer` inside WebAssembly, where that
full-window filter originally took several seconds. Disabling the filter made
the page readable and improved the frame, but the translucent scene still needs
expensive software rasterization.

## Previous workaround and its correction

`CpuRenderer` and `VelloRenderer` expose a backdrop-filter policy. The earlier
workaround disabled backdrop filters specifically in the Web Canvas2D presenter.
That default is removed for the 0.16.0 candidate: software Web rendering must
honour the same declared effects as GPU rendering and explicit screenshots.
The existing exact-root-clip optimisation retains its correctness guard; it
does not justify suppressing an effect. The explicit renderer policy remains
available to callers that deliberately request it.

Web Canvas2D performance diagnostics also report scene-recording,
rasterization, and RGBA-copy duration inside the total software-render time.

## Historical verification of the workaround

These results describe the earlier filter-disabled source, not the corrected
candidate. Re-measure the complete effect-preserving path before claiming a
performance improvement.

- `cargo test -p fission-render-vello --lib` passes all 52 renderer tests,
  including the check that the filter is skipped while the scrim path still
  paints.
- `FISSION_WEB_TEST_CONTROL=1 cargo check -p fission-shell-winit --target wasm32-unknown-unknown`
  passes.
- Application Web QA passed the modal-opening flow with
  `canvas2d-software`. The screenshot remains legible after removing blur. Its
  1,583.15 ms `softwareRenderMs` versus 3,957.025 ms before the patch is still
  above interactive response time.
- A repeat recorded 1,682.34 ms `softwareRenderMs`, with 213.53 ms
  scene recording, 1,418.60 ms rasterization, 0.005 ms output copy, and 0.975 ms
  canvas upload. A 4,308-sample Chrome CPU profile (about 5 s including polling
  and setup) concentrated the raster work in Vello CPU SIMD compositing kernels.
  The measured optimization target is the Vello CPU raster path.
