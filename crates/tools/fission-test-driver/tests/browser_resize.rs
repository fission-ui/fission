//! Real Chromium against an opt-in Rust/Fission fixture and a static DOM site.
//! See docs/browser-test-control.md for fixture setup; these tests never inject UI.
use fission_test_driver::{BrowserTestOptions, LiveTestClient, SelectorQuery};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn observe(c: &LiveTestClient) -> anyhow::Result<Value> {
    c.browser_evaluate_json("(() => {const c=document.querySelector('canvas'),r=c?.getBoundingClientRect();return {width:innerWidth,height:innerHeight,scale:devicePixelRatio,url:location.href,timeOrigin:performance.timeOrigin,frame:globalThis.__FISSION_TEST__?.frame||null,canvas:r?{width:r.width,height:r.height,pixel_width:c.width,pixel_height:c.height}:null};})()")
}
fn capture(c: &LiveTestClient, name: &str, width: u32) -> anyhow::Result<()> {
    let png = c.capture_screenshot_png()?;
    let image = image::load_from_memory(&png)?.to_rgba8();
    assert_eq!((image.width(), image.height()), (width, 900));
    let first = image.get_pixel(0, 0);
    assert!(image.pixels().any(|p| p != first), "uniform capture");
    if let Some(dir) = std::env::var_os("FISSION_RESIZE_OUTPUT") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{name}-{width}.png")), png)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires FISSION_RESIZE_WEB_URL, installed Chromium, and loopback sockets"]
fn canvas_same_session_state_route_frames_raw_bridge_timeout_and_recovery() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_RESIZE_WEB_URL")?;
    let mut options = BrowserTestOptions::new(&url).fission_canvas();
    options.timeout_ms = 3_000;
    let port = std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port();
    options.cdp_port = Some(port);
    let c = LiveTestClient::launch_browser(options)?;
    assert_eq!(c.browser_evaluate_json("(() => {const d=Object.getOwnPropertyDescriptor(__FISSION_TEST__,'frame');const snapshot=__FISSION_TEST__.frame;snapshot.width=-1;return {getter:typeof d.get,setter:typeof d.set,width:__FISSION_TEST__.frame.width,legacy:typeof globalThis.__FISSION_REVIEW_FRAME};})()")?, json!({"getter":"function","setter":"undefined","width":1280,"legacy":"undefined"}));
    let unrelated = LiveTestClient::launch_browser(BrowserTestOptions::new(&url).fission_canvas())?;
    let unrelated_initial = observe(&unrelated)?;
    c.activate_selector(SelectorQuery::label("Increment"))
        .map_err(|e| anyhow::anyhow!("increment: {e:#}"))?;
    c.activate_selector(SelectorQuery::label("Details"))
        .map_err(|e| anyhow::anyhow!("details: {e:#}"))?;
    let initial = observe(&c)?;
    assert_eq!(initial["url"], format!("{url}about/details"));
    for (step, width) in [1280, 390, 800, 1280].into_iter().enumerate() {
        let old = observe(&c)?;
        c.simulate_resize(width, 900)?;
        let v = observe(&c)?;
        assert_eq!(v["width"], width);
        assert_eq!(v["height"], 900);
        assert_eq!(v["scale"], 1);
        assert_eq!(v["canvas"]["width"], width);
        assert_eq!(v["canvas"]["height"], 900);
        assert_eq!(v["canvas"]["pixel_width"], width);
        assert_eq!(v["canvas"]["pixel_height"], 900);
        assert_eq!(v["frame"]["width"], width);
        assert_eq!(v["frame"]["height"], 900);
        assert_eq!(v["frame"]["phase"], "submitted");
        assert_eq!(v["url"], initial["url"]);
        assert_eq!(v["timeOrigin"], initial["timeOrigin"]);
        if old["width"] != width {
            assert!(v["frame"]["frame"].as_u64() > old["frame"]["frame"].as_u64());
        }
        let text = c.get_text()?;
        assert!(text.iter().any(|t| t.text == "Count: 1"));
        assert!(text.iter().any(|t| t.text == "Details route"));
        let active = if width < 600 {
            "Narrow layout"
        } else {
            "Wide layout"
        };
        let node = c
            .resolve_selector(SelectorQuery::label(active))
            .map_err(|e| anyhow::anyhow!("responsive {active}: {e:#}"))?;
        assert!(node.logical_bounds.width > 0.0);
        assert!(
            node.logical_bounds.x >= 0.0
                && node.logical_bounds.x + node.logical_bounds.width <= width as f32
        );
        let report = c.browser_report().unwrap();
        assert_eq!((report.width, report.height), (width, 900));
        capture(&c, &format!("web-step-{step}"), width)?;
        println!(
            "{}",
            json!({"resize":v,"state":"Count: 1","active":active,"semantic_bounds":node.logical_bounds})
        );
    }
    let before = observe(&c)?;
    for (w, h) in [(0, 900), (8193, 900), (8192, 8192), (u32::MAX, 1)] {
        assert!(c
            .simulate_resize(w, h)
            .unwrap_err()
            .to_string()
            .contains("invalid_viewport"));
        let v = observe(&c)?;
        assert_eq!(v["width"], before["width"]);
        assert_eq!(v["canvas"], before["canvas"]);
    }
    let request = c.browser_evaluate_json(
        "__FISSION_TEST__.submit(JSON.stringify({cmd:'SimulateResize',width:390,height:900}))",
    )?;
    let raw = c.browser_evaluate_json(&format!("JSON.parse(__FISSION_TEST__.poll({request}))"))?;
    assert_eq!(raw["status"], "Error");
    assert!(raw["message"]
        .as_str()
        .unwrap()
        .contains("client.simulate_resize(width, height)"));
    let after = observe(&c)?;
    assert_eq!(after["width"], before["width"]);
    assert_eq!(after["canvas"], before["canvas"]);
    println!("{}", json!({"raw_bridge":raw,"unchanged":after}));

    // Freeze only the acknowledgment in this disposable development page.
    // Actual Chromium/canvas resize still occurs, but cannot claim success.
    c.browser_evaluate_json("globalThis.__savedFrame=Object.getOwnPropertyDescriptor(__FISSION_TEST__,'frame');Object.defineProperty(__FISSION_TEST__,'frame',{value:{...__FISSION_TEST__.frame,width:390},writable:false,configurable:true});true")?;
    let start = Instant::now();
    let error = c.simulate_resize(390, 900).unwrap_err();
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(format!("{error:#}").contains("observed"));
    assert_eq!(c.browser_report().unwrap().width, 390);
    assert!(
        c.capture_screenshot_png().is_err(),
        "stale frame capture must fail"
    );
    assert!(
        c.simulate_resize(390, 900).is_err(),
        "same-size retry must retain stale-frame requirement"
    );
    println!(
        "{}",
        json!({"timeout":format!("{error:#}"),"elapsed_ms":start.elapsed().as_millis(),"observed":observe(&c)?})
    );
    c.browser_evaluate_json(
        "Object.defineProperty(__FISSION_TEST__,'frame',globalThis.__savedFrame);true",
    )?;
    c.simulate_resize(800, 900)?;
    c.simulate_resize(1280, 900)?;
    assert!(c.get_text()?.iter().any(|t| t.text == "Count: 1"));
    assert_eq!(observe(&c)?["timeOrigin"], initial["timeOrigin"]);
    capture(&c, "recovered", 1280)?;

    // Missing opt-in capability fails before mutating host or runtime.
    c.browser_evaluate_json("globalThis.__resizeBridge=globalThis.__FISSION_TEST__;delete globalThis.__FISSION_TEST__;true")?;
    assert!(format!("{:#}", c.simulate_resize(390, 900).unwrap_err()).contains("unsupported_host"));
    assert_eq!(observe(&c)?["width"], 1280);
    c.browser_evaluate_json("globalThis.__FISSION_TEST__=globalThis.__resizeBridge;true")?;
    c.simulate_resize(1280, 900)?;
    // Drop closes this owned session. A separate CDP test closes the target
    // itself; browsers can ignore a page-script window.close().
    drop(c);
    assert!(
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "owned Chromium CDP port leaked"
    );
    assert_eq!(
        observe(&unrelated)?["timeOrigin"],
        unrelated_initial["timeOrigin"]
    );
    assert_eq!(observe(&unrelated)?["width"], 1280);
    assert!(unrelated.get_text()?.iter().any(|t| t.text == "Count: 0"));
    println!("owned Chromium CDP port released; unrelated Chromium session remained intact");
    Ok(())
}

#[test]
#[ignore = "requires FISSION_RESIZE_DOM_URL, installed Chromium, and loopback sockets"]
fn static_dom_host_resize_without_bridge_and_scale_contract() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_RESIZE_DOM_URL")?;
    let c = LiveTestClient::launch_browser(BrowserTestOptions::new(url))?;
    let initial = observe(&c)?;
    assert_eq!(
        c.browser_evaluate_json("typeof globalThis.__FISSION_TEST__")?,
        "undefined"
    );
    assert!(c
        .get_text()
        .unwrap_err()
        .to_string()
        .contains("unsupported_page_mode"));
    assert!(c
        .press_key("Tab", 0)
        .unwrap_err()
        .to_string()
        .contains("unsupported_page_mode"));
    for (step, width) in [1280, 390, 800, 1280].into_iter().enumerate() {
        c.simulate_resize(width, 900)?;
        let v = observe(&c)?;
        assert_eq!(v["width"], width);
        assert_eq!(v["scale"], 1);
        assert_eq!(v["url"], initial["url"]);
        assert_eq!(v["timeOrigin"], initial["timeOrigin"]);
        assert_eq!(v["frame"], Value::Null);
        assert_eq!(c.browser_report().unwrap().width, width);
        capture(&c, &format!("dom-step-{step}"), width)?;
        println!("{}", json!({"dom_resize":v}));
    }
    Ok(())
}
