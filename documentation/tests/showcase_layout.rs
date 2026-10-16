use fission_test_driver::{BrowserTestOptions, LiveTestClient};
use serde_json::Value;

#[test]
#[ignore = "requires FISSION_SHOWCASE_PAGE_URL and installed Chromium"]
fn showcase_fills_the_viewport_below_the_header() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_SHOWCASE_PAGE_URL")?;
    let mut options = BrowserTestOptions::new(url);
    options.viewport_width = 1280;
    options.viewport_height = 900;

    let browser = LiveTestClient::launch_browser(options)?;
    let layout: Value = browser.browser_evaluate_json(
        r#"(() => {
            const iframe = document.querySelector('.fission-site-showcase-frame iframe');
            const header = document.querySelector('.fission-site-showcase-page .fission-site-home-header');
            if (!iframe || !header) return null;
            const frame = iframe.getBoundingClientRect();
            const navigation = header.getBoundingClientRect();
            return {
                viewportHeight: window.innerHeight,
                headerBottom: navigation.bottom,
                frameTop: frame.top,
                frameBottom: frame.bottom,
                frameHeight: frame.height
            };
        })()"#,
    )?;

    assert!(
        !layout.is_null(),
        "showcase iframe or navigation was not rendered"
    );
    let viewport_height = layout["viewportHeight"].as_f64().unwrap_or_default();
    let header_bottom = layout["headerBottom"].as_f64().unwrap_or_default();
    let frame_top = layout["frameTop"].as_f64().unwrap_or_default();
    let frame_bottom = layout["frameBottom"].as_f64().unwrap_or_default();
    let frame_height = layout["frameHeight"].as_f64().unwrap_or_default();

    println!("showcase viewport layout: {layout}");

    assert!(
        (frame_top - header_bottom).abs() <= 1.0,
        "showcase must begin immediately below the header: {layout}"
    );
    assert!(
        (frame_bottom - viewport_height).abs() <= 1.0,
        "showcase must reach the bottom of the initial viewport: {layout}"
    );
    assert!(
        frame_height >= viewport_height * 0.75,
        "showcase collapsed instead of filling the page body: {layout}"
    );

    Ok(())
}
