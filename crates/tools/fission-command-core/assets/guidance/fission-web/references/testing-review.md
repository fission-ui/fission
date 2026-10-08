<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Build, browser testing and review

Format and run the narrow compile/test that exercises the changed API.
Static sites use `fission site check --project-dir .` (renders routes),
`site build`, `site routes`, and `site serve --no-open --port 8123`.
Web uses `fission build --target web --project-dir .`,
`fission run --target web --project-dir . --no-open --port 8123`, and
`fission test --target web --project-dir .`. Web build/run/test accept
`--features` and `--no-default-features`.

CLI test is a smoke test, not a complete UX review. Use the version-aligned
`fission-test-driver` Rust API for browser captures:

```rust
use fission_test_driver::{run_browser_smoke, BrowserTestOptions};
fn capture_static(url: &str, path: &str) -> anyhow::Result<()> {
    let mut options = BrowserTestOptions::new(url).screenshot(path);
    options.viewport_width = 390;
    options.viewport_height = 844;
    let report = run_browser_smoke(options)?;
    assert_eq!(report.screenshot_path.as_deref(), Some(std::path::Path::new(path)));
    Ok(())
}
```

For interactive Web, build with the compile-time environment switch
`FISSION_WEB_TEST_CONTROL=1 fission build --target web --project-dir .` and
serve that scaffold, then use:

```rust
use fission_test_driver::{BrowserTestOptions, LiveTestClient, SelectorQuery};
fn exercise_web(url: &str, screenshot_path: &str) -> anyhow::Result<()> {
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;
    client.wait_for_selector(SelectorQuery::semantic_identifier("disclosure.toggle"), 5_000)?;
    client.tap_selector(SelectorQuery::semantic_identifier("disclosure.toggle"))?;
    client.screenshot(screenshot_path)?;
    Ok(())
}
```

Set stable semantic identifiers in authored widgets. Text/label selectors can
change with locale and duplicate content; scope/disambiguate when needed.
Screenshot methods write files: keep the actual path and open the image.
The browser driver owns its disposable Chromium process/profile; dropping the
client closes it.

Browser smoke fails on reported resource errors, including missing favicons.
On this base, a Web scaffold served from `platforms/web/` can request
`/assets/app-icon.png` while the generated icon lives at project
`assets/app-icon.png`. Stage that real icon under `platforms/web/assets/` or
configure an actually served favicon URL. The minimal custom static page also
needs a served favicon if Chromium requests `/favicon.ico`. Fix the missing
resource rather than suppressing the browser error.

| Readiness level in this base | Evidence |
| --- | --- |
| DOM smoke | Document/body readiness |
| FissionCanvas smoke | Canvas readiness and an identified renderer |
| LiveTest launch | Above plus test bridge availability |
| Product assertions and inspected screenshots | Only tested behavior/appearance |

Smoke readiness does not prove all layout/paint. Semantic logical/visible bounds
cover supported semantic nodes, not every primitive; verify pixels and reachable
controls too. A blank capture is a failed visual check even when semantic
assertions pass; retrying capture does not establish paint readiness.
For responsive UI launch separate real browser sessions at mobile
and desktop viewports; inspect overflow, state, focus and routes.

Serve through `fission run --target web` or `fission site serve`. Inspect
`fission test --help` for the installed testing/review capabilities; this base
provides smoke testing. Do not assume standalone `fission preview` or
`fission review` interfaces.

This base has no verified same-session browser resize command or generic
test-control frame acknowledgment guarantee. Raw Web `SimulateResize` is not proof of
an actual browser resize. Separate sessions do not prove preserved in-session
state. Native HTTP `/cmd` examples do not automatically apply to Web's bridge.

Inspect `fission <command> --help` before automating flags. General build/run
failures here use human output; only `skills check|update --json` has the
guidance schema. Those fields do not define a generic build/run/test result
contract. Its recovery argument arrays go directly to a process API;
do not concatenate them into shell strings.
