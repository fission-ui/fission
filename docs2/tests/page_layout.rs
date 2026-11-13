//! Check the CSS cascade on real generated pages, not just authored Text sizes.
use anyhow::{Context, Result};
use fission_test_driver::{BrowserTestOptions, LiveTestClient};

#[test]
#[ignore = "requires FISSION_DOCS2_QA_URL, a built Docs2 site and Chromium"]
fn goal_pages_keep_their_heading_scale_and_responsive_regions() -> Result<()> {
    let base = std::env::var("FISSION_DOCS2_QA_URL").context("set FISSION_DOCS2_QA_URL")?;
    for path in ["/", "/docs/", "/blog/build-a-login-page-in-rust/"] {
        let mut options =
            BrowserTestOptions::new(format!("{}{path}", base.trim_end_matches('/'))).website();
        options.viewport_width = 1536;
        options.viewport_height = 1024;
        let browser = LiveTestClient::launch_browser(options)?;
        for width in [1536, 390] {
            browser.simulate_resize(width, 1024)?;
            let layout = browser.browser_evaluate_json(
                r#"(() => {
                    const heading = document.querySelector('h1 .fission-site-text-run');
                    const finished = document.querySelector('[data-fission-semantics="goal-finished-preview"] iframe');
                    return {
                        width: innerWidth,
                        usableWidth: document.documentElement.clientWidth,
                        scrollWidth: document.documentElement.scrollWidth,
                        headingSize: parseFloat(getComputedStyle(heading).fontSize),
                        cards: [...document.querySelectorAll('[data-fission-semantics="hub-task-card"]')].map(card => ({
                            height: card.getBoundingClientRect().height,
                            border: parseFloat(getComputedStyle(card).borderTopWidth)
                        })),
                        finished: finished ? {
                            width: finished.getBoundingClientRect().width,
                            height: finished.getBoundingClientRect().height,
                            visible: !finished.closest('details:not([open])'),
                            title: finished.title,
                            url: finished.src
                        } : null,
                        falseStableClaim: document.body.innerText.includes('Latest stable release')
                    };
                })()"#,
            )?;
            println!("Docs2 {path} at {width}px: {layout}");
            assert!(
                layout["scrollWidth"].as_f64().unwrap()
                    <= layout["usableWidth"].as_f64().unwrap() + 1.0,
                "{layout}"
            );
            let heading_size = layout["headingSize"].as_f64().unwrap();
            let expected = if path == "/" && width > 700 {
                48.0
            } else {
                36.0
            };
            assert!((heading_size - expected).abs() < 0.1, "{layout}");
            assert_eq!(layout["falseStableClaim"], false);
            if path == "/docs/" {
                let cards = layout["cards"].as_array().unwrap();
                assert_eq!(cards.len(), 4);
                assert!(cards
                    .iter()
                    .all(|card| card["border"].as_f64().unwrap() > 0.0));
                if width > 700 {
                    let first_height = cards[0]["height"].as_f64().unwrap();
                    assert!(
                        cards.iter().all(|card| {
                            (card["height"].as_f64().unwrap() - first_height).abs() <= 1.0
                        }),
                        "{layout}"
                    );
                }
            }
            if path.contains("login") {
                let finished = &layout["finished"];
                assert!(finished["width"].as_f64().unwrap() > 200.0);
                assert!(finished["height"].as_f64().unwrap() >= 440.0);
                assert_eq!(finished["visible"], true);
                assert!(finished["url"]
                    .as_str()
                    .unwrap()
                    .contains("/rust-login-page/main-flow/"));
                assert!(!finished["title"].as_str().unwrap().is_empty());
            }
        }
    }
    Ok(())
}
