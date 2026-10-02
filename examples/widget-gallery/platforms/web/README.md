# Web target

The browser target renders the same retained widget gallery used by the desktop
example. It is the real-shell visual qualification surface for responsive,
interaction-state, and golden-image tests.

- Run `fission doctor web --project-dir .` to check the Web toolchain.
- Run `fission run --target web --project-dir .` to build and open the gallery.
- Run `./platforms/web/run-browser.sh` to build and serve it directly.
- Set `FISSION_WEB_PORT=<port>` when port `8123` is already in use.

## Visual baselines

The suite starts with a 1486×1058 guided-flow product surface held against an
independently rendered design oracle. It then covers 24 browser-rendered
regression baselines: desktop and narrow viewports, light and dark themes, and
six states. The states cover the resting page, focus on a text field, pointer
hover on a button, and open menu, select, and dialog surfaces. Before capturing
an interaction state, the test uses the semantic test identifier to bring its
target into view and apply the interaction.

The test pins the WebGPU renderer, uses a device scale factor of 1, requests
reduced motion, and verifies the requested viewport and visible overlay. The 24
renderer-owned snapshots use exact pixel comparison. The independent
guided-flow oracle has a bounded per-channel allowance for rasterizer and glyph
outline differences plus a 4% maximum mean RGB error, so broad colour or layout
drift cannot hide inside the cross-rasterizer allowance. The suite writes the
actual image for every case and a red heatmap for each mismatch under
`.artifacts/visual-baselines/widget-gallery/`.

First build with the test-only browser bridge and serve the example. From
`examples/widget-gallery`:

```bash
FISSION_WEB_TEST_CONTROL=1 ./platforms/web/build-wasm.sh
python3 -m http.server 8129 --bind 127.0.0.1 --directory .
```

Then compare all approved baselines from the repository root:

```bash
FISSION_QUALITY_GALLERY_URL='http://127.0.0.1:8129/platforms/web/' \
  cargo test --locked -p widget-gallery --test quality_visual_baselines \
  quality_gallery_matches_approved_browser_baselines \
  -- --ignored --exact --nocapture
```

Set `FISSION_CHROME` when the supported browser is not on a standard executable
path. Set `FISSION_VISUAL_ARTIFACT_DIR` to place mismatch evidence elsewhere.

Baseline updates are always explicit. After reviewing the actual images and
confirming that the visual change is intentional, regenerate the complete
24-image renderer-regression set with:

```bash
FISSION_QUALITY_GALLERY_URL='http://127.0.0.1:8129/platforms/web/' \
FISSION_UPDATE_WIDGET_GALLERY_GOLDENS=1 \
  cargo test --locked -p widget-gallery --test quality_visual_baselines \
  quality_gallery_matches_approved_browser_baselines \
  -- --ignored --exact --nocapture
```

Review the updated files in
`tests/goldens/quality-gallery/webgpu-vello/` before committing them. Do not
update baselines merely to silence an unexplained mismatch. Update mode never
replaces the guided-flow design oracle with the current Fission rendering.

The Web CI job builds this same test-control package, serves it on loopback,
uses the test driver's fixed supported-browser selection order, records the
runtime identity, and invokes the ignored baseline test by exact name. When a
comparison fails, CI uploads the actual images, diff heatmaps, runtime identity,
and server log as one diagnostic artifact.
