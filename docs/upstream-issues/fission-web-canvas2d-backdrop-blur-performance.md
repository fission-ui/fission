# Web Canvas2D fallback spends seconds rendering modal backdrop blur

Status: the Web Canvas2D fallback omits backdrop filters while preserving the
translucent modal scrim and foreground content. Zoffice's New Document modal
frame fell from 3,987.68 ms to 1,618.5 ms, but CPU rendering still takes
1,583.15 ms and remains too slow. The frame has 359 nodes, with build, layout,
and prepare taking about 4 ms combined. Additional CPU stage timings split
scene recording, rasterization, and output copying to locate the remaining cost.

## Reproduction and measurements

Zoffice measured the Fission Web target at 1280×900 with `web-perf` enabled and
`canvas2d-software` selected. Opening the New Document modal took 3,987.68 ms
for the frame: 3,957.025 ms in software rasterization and 0.975 ms in Canvas
upload. The software raster stage, rather than canvas presentation, dominates
the delay.

The modal scrim applies a 4 px backdrop blur. Fission's Web Canvas2D presenter
renders the retained scene through `CpuRenderer` inside WebAssembly, where that
full-window filter takes several seconds. This makes a supported software
fallback feel unresponsive when opening a modal.

## Fission change

`CpuRenderer` and `VelloRenderer` now expose a backdrop-filter policy. The Web
Canvas2D presenter disables backdrop filters so software rendering still paints
the modal's translucent scrim and foreground content without the expensive
full-window blur. The regular CPU renderer defaults to preserving filters, so
explicit screenshot rendering remains faithful; GPU rendering is unchanged.
Web Canvas2D performance diagnostics also report scene-recording,
rasterization, and RGBA-copy duration inside the total software-render time.

## Verification

- `cargo test -p fission-render-vello --lib` passes all 52 renderer tests,
  including the check that the filter is skipped while the scrim path still
  paints.
- `FISSION_WEB_TEST_CONTROL=1 cargo check -p fission-shell-winit --target wasm32-unknown-unknown`
  passes.
- Zoffice's Web QA passed the New Document modal flow on `46843123` with
  `canvas2d-software`. The screenshot remains legible after removing blur. Its
  1,583.15 ms `softwareRenderMs` versus 3,957.025 ms before the patch is still
  above interactive response time; the new substage diagnostics need a rerun
  before choosing further rendering tradeoffs.
