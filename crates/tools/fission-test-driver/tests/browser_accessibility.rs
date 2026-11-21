//! Real Chromium coverage for the Web shell's browser-native accessibility DOM.
//! See docs/browser-test-control.md for the opt-in Web app setup.

use fission_test_driver::{BrowserTestOptions, LiveTestClient, detect_chrome};
use serde_json::json;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires FISSION_ACCESSIBILITY_WEB_URL, the opt-in Web test bridge, and Chromium"]
fn web_accessibility_controls_forward_activation_and_text_input() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser accessibility test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let url = std::env::var("FISSION_ACCESSIBILITY_WEB_URL")?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;
    client.wait_for_text("Fission web smoke", 30_000)?;

    let projection = client.browser_evaluate_json(
        r#"(() => {
            const canvas = document.querySelector('canvas');
            const root = document.querySelector('[data-fission-accessibility-overlay]');
            const button = root?.querySelector('[data-fission-semantics="web-smoke.increment"]');
            const input = root?.querySelector('[data-fission-semantics="web-smoke.text.primary"]');
            const slider = root?.querySelector('[data-fission-semantics="web-smoke.slider"]');
            const style = button ? getComputedStyle(button) : null;
            return {
                canvasHidden: canvas?.getAttribute('aria-hidden') === 'true',
                rootPresent: !!root,
                buttonTag: button?.tagName ?? null,
                buttonName: button?.getAttribute('aria-label') ?? button?.getAttribute('aria-labelledby') ?? null,
                buttonStyle: style ? {
                    borderWidth: style.borderTopWidth,
                    background: style.backgroundColor,
                    color: style.color,
                    pointerEvents: style.pointerEvents,
                } : null,
                inputTag: input?.tagName ?? null,
                inputName: input?.getAttribute('aria-label') ?? null,
                sliderTag: slider?.tagName ?? null,
                sliderName: slider?.getAttribute('aria-label') ?? null,
            };
        })()"#,
    )?;
    assert_eq!(
        projection,
        json!({
            "canvasHidden": true,
            "rootPresent": true,
            "buttonTag": "BUTTON",
            "buttonName": "Increment",
            "buttonStyle": {
                "borderWidth": "0px",
                "background": "rgba(0, 0, 0, 0)",
                "color": "rgba(0, 0, 0, 0)",
                "pointerEvents": "auto",
            },
            "inputTag": "INPUT",
            "inputName": "Primary field",
            "sliderTag": "INPUT",
            "sliderName": "Smoke test slider",
        })
    );

    let screenshot = client.capture_screenshot_png()?;
    let decoded = image::load_from_memory(&screenshot)?.to_rgba8();
    assert!(decoded.width() > 0 && decoded.height() > 0);
    let canvas = client.browser_evaluate_json(
        r#"(() => {
            const rect = document.querySelector('canvas')?.getBoundingClientRect();
            return rect ? {x: rect.left, y: rect.top, width: rect.width, height: rect.height, scale: devicePixelRatio} : null;
        })()"#,
    )?;
    let scale = canvas["scale"].as_f64().unwrap_or(1.0);
    let x = (canvas["x"].as_f64().unwrap_or(0.0) * scale)
        .floor()
        .max(0.0) as u32;
    let y = (canvas["y"].as_f64().unwrap_or(0.0) * scale)
        .floor()
        .max(0.0) as u32;
    let width = (canvas["width"].as_f64().unwrap_or(0.0) * scale)
        .ceil()
        .max(1.0) as u32;
    let height = (canvas["height"].as_f64().unwrap_or(0.0) * scale)
        .ceil()
        .max(1.0) as u32;
    anyhow::ensure!(
        x + width <= decoded.width() && y + height <= decoded.height(),
        "canvas screenshot bounds must fit inside the captured PNG"
    );
    let canvas_pixels = image::imageops::crop_imm(&decoded, x, y, width, height).to_image();
    let distinct_canvas_colors = canvas_pixels
        .pixels()
        .map(|pixel| pixel.0)
        .collect::<HashSet<_>>();
    assert!(
        distinct_canvas_colors.len() > 8,
        "the screenshot's canvas region should contain rendered Fission content"
    );
    if let Some(directory) = std::env::var_os("FISSION_ACCESSIBILITY_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("web-accessibility-initial.png"), &screenshot)?;
    }

    assert_eq!(
        client.browser_evaluate_json(
            r#"(() => {
                const button = document.querySelector('[data-fission-semantics="web-smoke.increment"]');
                if (!button) return false;
                button.click();
                return true;
            })()"#,
        )?,
        json!(true),
    );
    client.wait_for_text("Count: 1", 5_000)?;

    assert_eq!(
        client.browser_evaluate_json(
            r#"(() => {
                const input = document.querySelector('[data-fission-semantics="web-smoke.text.primary"]');
                if (!input) return false;
                input.focus();
                return document.activeElement === input;
            })()"#,
        )?,
        json!(true),
    );
    assert_eq!(
        client.browser_evaluate_json(
            r#"(() => {
                const input = document.querySelector('[data-fission-semantics="web-smoke.text.primary"]');
                if (!input) return false;
                input.value = 'web bridge';
                input.setSelectionRange(input.value.length, input.value.length);
                return input.dispatchEvent(new InputEvent('input', {
                    bubbles: true,
                    inputType: 'insertText',
                    data: 'web bridge',
                    isComposing: false,
                }));
            })()"#,
        )?,
        json!(true),
    );
    client.wait_for_text("Primary value: web bridge", 10_000)?;
    assert_eq!(
        client.browser_evaluate_json(
            r#"document.querySelector('[data-fission-semantics="web-smoke.text.primary"]')?.value ?? null"#,
        )?,
        json!("web bridge"),
        "the native control should stay synchronized with Fission state"
    );

    assert_eq!(
        client.browser_evaluate_json(
            r#"(() => {
                const slider = document.querySelector('[data-fission-semantics="web-smoke.slider"]');
                if (!slider) return false;
                slider.value = '73';
                return slider.dispatchEvent(new InputEvent('input', {
                    bubbles: true, inputType: 'insertText', data: '73', isComposing: false,
                }));
            })()"#,
        )?,
        json!(true),
    );
    client.wait_for_text("Slider value: 73", 10_000)?;

    assert_eq!(
        client.browser_evaluate_json(
            r#"(() => {
                const input = document.querySelector('[data-fission-semantics="web-smoke.text.primary"]');
                if (!input) return null;
                const event = new KeyboardEvent('keydown', {
                    key: 'Tab', shiftKey: true, bubbles: true, cancelable: true,
                });
                input.dispatchEvent(event);
                return {prevented: event.defaultPrevented, before: input.getAttribute('data-fission-semantics')};
            })()"#,
        )?,
        json!({
            "prevented": true,
            "before": "web-smoke.text.primary",
        }),
        "Tab must be handled by Fission focus traversal instead of browser DOM order"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let focused = client.browser_evaluate_json(
            r#"document.activeElement?.getAttribute('data-fission-semantics') ?? null"#,
        )?;
        if focused != json!("web-smoke.text.primary") {
            assert!(
                focused.is_string(),
                "focus should move to another semantic node"
            );
            break;
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "Fission did not apply Shift+Tab focus"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}
