//! Rendered docs2 qualification, not a test of the legacy documentation template.
//! Serve docs2/dist/site with scripts/serve_docs2_qa.py, then set
//! FISSION_SHOWCASE_PAGE_URL=http://127.0.0.1:6467/showcase/ and run with --ignored.
use anyhow::{Context, Result};
use fission_test_driver::{BrowserTestOptions, LiveTestClient};
use serde_json::Value;
use std::time::{Duration, Instant};

const OBSERVE_LAYOUT: &str = r#"(() => {
    const page = document.querySelector('[data-fission-semantics="site-showcase-page"]');
    const iframe = page?.querySelector('[data-fission-semantics="site-showcase-frame"] iframe');
    const header = page?.querySelector('[data-fission-semantics="site-header"]');
    if (!iframe || !header) return null;
    const frame = iframe.getBoundingClientRect();
    const navigation = header.getBoundingClientRect();
    const child = iframe.contentWindow;
    const canvas = child?.document.querySelector('canvas');
    const canvasBounds = canvas?.getBoundingClientRect();
    return {
        viewportWidth: innerWidth,
        viewportHeight: innerHeight,
        scrollWidth: document.documentElement.scrollWidth,
        headerTop: navigation.top,
        headerBottom: navigation.bottom,
        frameTop: frame.top,
        frameBottom: frame.bottom,
        frameLeft: frame.left,
        frameRight: frame.right,
        frameWidth: frame.width,
        frameHeight: frame.height,
        frameTitle: iframe.title,
        pageTimeOrigin: performance.timeOrigin,
        appTimeOrigin: child?.performance.timeOrigin,
        appLocation: child?.location.href,
        appViewportWidth: child?.innerWidth,
        appViewportHeight: child?.innerHeight,
        canvasWidth: canvasBounds?.width ?? 0,
        canvasHeight: canvasBounds?.height ?? 0
    };
})()"#;

fn observe_rendered_layout(browser: &LiveTestClient) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let layout = browser.browser_evaluate_json(OBSERVE_LAYOUT)?;
        if layout["canvasWidth"].as_f64().unwrap_or_default() > 0.0
            && layout["canvasHeight"].as_f64().unwrap_or_default() > 0.0
            && (number(&layout, "canvasWidth") - number(&layout, "frameWidth")).abs() <= 1.0
            && (number(&layout, "canvasHeight") - number(&layout, "frameHeight")).abs() <= 1.0
        {
            return Ok(layout);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "docs2 showcase did not load its real canvas app: {layout}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn number(layout: &Value, key: &str) -> f64 {
    layout[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing numeric {key} in showcase layout: {layout}"))
}

fn verify_layout(layout: &Value, width: u32, height: u32) {
    println!("docs2 showcase viewport layout: {layout}");
    let viewport_width = number(layout, "viewportWidth");
    let viewport_height = number(layout, "viewportHeight");
    let header_bottom = number(layout, "headerBottom");
    assert_eq!(viewport_width, f64::from(width), "{layout}");
    assert_eq!(viewport_height, f64::from(height), "{layout}");
    assert!(number(layout, "headerTop").abs() <= 1.0, "{layout}");
    assert!(
        header_bottom > 0.0 && header_bottom < viewport_height / 3.0,
        "header must leave the app most of the viewport: {layout}"
    );
    assert!(
        (number(layout, "frameTop") - header_bottom).abs() <= 1.0,
        "showcase must start immediately below the shared header: {layout}"
    );
    assert!(
        (number(layout, "frameBottom") - viewport_height).abs() <= 1.0,
        "showcase must reach the viewport bottom: {layout}"
    );
    assert!(
        number(layout, "frameHeight") >= viewport_height * 0.65,
        "showcase must not collapse to an iframe's intrinsic height: {layout}"
    );
    assert!(
        number(layout, "frameLeft").abs() <= 1.0
            && (number(layout, "frameRight") - viewport_width).abs() <= 1.0
            && number(layout, "scrollWidth") <= viewport_width + 1.0,
        "showcase must fill the width without horizontal page overflow: {layout}"
    );
    assert!(
        (number(layout, "canvasWidth") - number(layout, "frameWidth")).abs() <= 1.0
            && (number(layout, "canvasHeight") - number(layout, "frameHeight")).abs() <= 1.0,
        "the real app's canvas must fill its iframe after layout settles: {layout}"
    );
    assert!(
        layout["frameTitle"]
            .as_str()
            .is_some_and(|title| !title.is_empty()),
        "showcase frame needs its accessible label: {layout}"
    );
    assert!(
        layout["appLocation"].as_str().is_some_and(|url| {
            url.contains("/showcase-app/") && url.contains("fission_renderer=canvas2d-software")
        }),
        "showcase must run the actual built example app: {layout}"
    );
}

fn screenshot_if_requested(browser: &LiveTestClient, name: &str) -> Result<()> {
    if let Some(directory) = std::env::var_os("FISSION_DOCS2_QA_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{name}.png"));
        browser.screenshot(path.to_str().context("QA screenshot path is not UTF-8")?)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires FISSION_SHOWCASE_PAGE_URL, built docs2/showcase assets and Chromium"]
fn showcase_fills_viewport_across_same_session_desktop_and_narrow_resizes() -> Result<()> {
    let url = std::env::var("FISSION_SHOWCASE_PAGE_URL").context("set the docs2 showcase URL")?;
    let browser = LiveTestClient::launch_browser(BrowserTestOptions::new(url).website())?;
    let first = observe_rendered_layout(&browser)?;
    for width in [1280, 390, 800, 1280] {
        browser.simulate_resize(width, 900)?;
        let layout = observe_rendered_layout(&browser)?;
        verify_layout(&layout, width, 900);
        assert_eq!(layout["pageTimeOrigin"], first["pageTimeOrigin"]);
        assert_eq!(
            layout["appTimeOrigin"], first["appTimeOrigin"],
            "resizing must not reload the embedded application"
        );
        screenshot_if_requested(&browser, &format!("showcase-desktop-{width}"))?;
    }
    Ok(())
}

#[test]
#[ignore = "requires FISSION_SHOWCASE_PAGE_URL, built docs2/showcase assets and Chromium"]
fn showcase_fills_a_real_mobile_emulated_viewport() -> Result<()> {
    let url = std::env::var("FISSION_SHOWCASE_PAGE_URL").context("set the docs2 showcase URL")?;
    let mut options = BrowserTestOptions::new(url).website().mobile(1.0);
    options.viewport_width = 390;
    options.viewport_height = 844;
    let browser = LiveTestClient::launch_browser(options)?;
    let layout = observe_rendered_layout(&browser)?;
    verify_layout(&layout, 390, 844);
    screenshot_if_requested(&browser, "showcase-mobile-390")?;
    Ok(())
}
