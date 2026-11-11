# Web Canvas2D fallback spends seconds rendering modal backdrop blur

Status: the Web Canvas2D fallback omits backdrop filters while preserving the
translucent modal scrim and foreground content. The latest Zoffice run on
`bcec7df5` measured 1,682.34 ms in software rendering and 1,656.85 ms for the
whole modal frame. Scene recording took 213.53 ms and CPU rasterization took
1,418.60 ms; output copy was negligible. Rasterization still dominates and
remains too slow for an interactive modal.

## Reproduction and measurements

Zoffice measured the Fission Web target at 1280×900 with `web-perf` enabled and
`canvas2d-software` selected. Opening the New Document modal took 3,987.68 ms
for the frame: 3,957.025 ms in software rasterization and 0.975 ms in Canvas
upload. The software raster stage, rather than canvas presentation, dominates
the delay.

The modal scrim applies a 4 px backdrop blur. Fission's Web Canvas2D presenter
renders the retained scene through `CpuRenderer` inside WebAssembly, where that
full-window filter originally took several seconds. Disabling the filter made
the page readable and improved the frame, but the translucent scene still needs
expensive software rasterization.

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
  above interactive response time.
- A repeat on `bcec7df5` recorded 1,682.34 ms `softwareRenderMs`, with 213.53 ms
  scene recording, 1,418.60 ms rasterization, 0.005 ms output copy, and 0.975 ms
  canvas upload. A 4,308-sample Chrome CPU profile (about 5 s including polling
  and setup) concentrated the raster work in Vello CPU SIMD compositing kernels.
  The measured optimization target is the Vello CPU raster path.
