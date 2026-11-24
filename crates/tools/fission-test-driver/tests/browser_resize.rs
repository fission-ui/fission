//! Real Chromium against an opt-in Rust/Fission fixture and a static DOM site.
//! See docs/browser-test-control.md for fixture setup; these tests never inject UI.
use anyhow::Context;
use fission_test_driver::{BrowserTestOptions, LiveTestClient, SelectorQuery};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn observe(c: &LiveTestClient) -> anyhow::Result<Value> {
    c.browser_evaluate_json("(() => {const c=document.querySelector('canvas'),r=c?.getBoundingClientRect(),raw=globalThis.__FISSION_TEST__?.frame;return {width:innerWidth,height:innerHeight,scale:devicePixelRatio,url:location.href,timeOrigin:performance.timeOrigin,frame:raw?JSON.parse(raw):null,canvas:r?{width:r.width,height:r.height,pixel_width:c.width,pixel_height:c.height}:null};})()")
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

fn capture_webgpu_frame(
    c: &LiveTestClient,
    name: &str,
    width: u32,
    height: u32,
) -> anyhow::Result<()> {
    let png = c
        .capture_screenshot_png()
        .with_context(|| format!("capture {name} at {width}x{height}"))?;
    let image = image::load_from_memory(&png)?.to_rgba8();
    assert_eq!((image.width(), image.height()), (width, height));
    let first = image.get_pixel(0, 0);
    assert!(
        image.pixels().any(|pixel| pixel != first),
        "WebGPU screenshot should contain Fission's rendered frame, not a blank browser capture"
    );
    if let Some(dir) = std::env::var_os("FISSION_RESIZE_OUTPUT") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{name}-{width}.png")), png)?;
    }
    Ok(())
}

fn collect_bounded_bridge_errors(client: &LiveTestClient, requests: &[u32]) -> anyhow::Result<()> {
    assert_eq!(requests.len(), 1024);
    assert!(requests.iter().all(|id| *id != 0));
    assert_eq!(
        requests
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1024
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut pending = requests.to_vec();
    let mut collected = std::collections::HashSet::new();
    while !pending.is_empty() {
        let ids = serde_json::to_string(&pending)?;
        let replies = client.browser_evaluate_json(&format!(
            r#"(() => {{
            const bridge = globalThis.__FISSION_TEST__;
            return {ids}.map(id => {{
                const raw = bridge.poll(id);
                return {{id, transport: typeof raw, response: JSON.parse(raw)}};
            }});
        }})()"#
        ))?;
        pending.clear();
        for reply in replies.as_array().expect("bridge response batch") {
            let id = reply["id"].as_u64().expect("request id") as u32;
            assert_eq!(reply["transport"], "string");
            if reply["response"].is_null() {
                pending.push(id);
            } else {
                assert_eq!(
                    reply["response"]["status"], "Error",
                    "request {id}: {reply}"
                );
                assert!(
                    reply["response"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("raw Web SimulateResize cannot control Chromium"),
                    "request {id} must produce its actual dispatched error: {reply}"
                );
                assert!(collected.insert(id), "request {id} was collected twice");
            }
        }
        anyhow::ensure!(
            pending.is_empty() || Instant::now() < deadline,
            "timed out collecting deferred bridge errors: {} collected, {} pending",
            collected.len(),
            pending.len()
        );
        if !pending.is_empty() {
            // A CDP evaluation is synchronous; return to the browser event loop
            // before polling again so its deferred dispatch tasks can run.
            std::thread::sleep(Duration::from_millis(8));
        }
    }
    assert_eq!(collected.len(), requests.len());
    let ids = serde_json::to_string(requests)?;
    let consumed = client.browser_evaluate_json(&format!(
        r#"(() => {{
        const bridge = globalThis.__FISSION_TEST__;
        return {ids}.every(id => {{
            const response = JSON.parse(bridge.poll(id));
            return response?.status === 'Error'
                && response.message === `unknown browser test request ${{id}}`;
        }});
    }})()"#
    ))?;
    assert_eq!(
        consumed, true,
        "completed responses must be removed after one collection"
    );
    Ok(())
}

#[test]
#[ignore = "requires FISSION_BRIDGE_WEB_URL, the opt-in Web test bridge, and Chromium"]
fn web_bridge_json_round_trips_remain_bounded_after_repeated_queries() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_BRIDGE_WEB_URL")?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;

    for _ in 0..128 {
        assert!(client
            .get_text()?
            .iter()
            .any(|item| item.text == "Fission web smoke"));
    }

    // Bridge payloads stay as JSON strings across the Wasm boundary, including nested semantic
    // trees and selector responses used by the real application.
    assert!(!client.get_tree()?.is_empty());
    let increment = client.resolve_selector(SelectorQuery::label("Increment"))?;
    assert_eq!(increment.label.as_deref(), Some("Increment"));

    // Reproduce repeated missing-selector responses followed by a pump. In a large
    // application, this pattern reached the bridge trap around request 51. Go well beyond that
    // point, then exercise each bridge accessor again in the same Wasm instance.
    for _ in 0..128 {
        assert!(client
            .resolve_selector(SelectorQuery::label("No matching widget"))
            .is_err());
        client.pump()?;
        assert!(client
            .browser_evaluate_json("typeof globalThis.__FISSION_TEST__.submit === 'function' && typeof globalThis.__FISSION_TEST__.poll === 'function'")?
            .as_bool()
            .unwrap());
    }
    let poll_diagnostics = client
        .browser_evaluate_json("globalThis.__FISSION_TEST__.pollDiagnostics()")?
        .as_u64()
        .unwrap();
    assert_eq!(poll_diagnostics & 0b11, 3);
    let poll_diagnostics_from_memory = client.browser_evaluate_json(
        "(() => { const bridge=globalThis.__FISSION_TEST__; return new DataView(bridge.memory.buffer).getUint32(bridge.pollDiagnosticsAddress,true); })()",
    )?.as_u64().unwrap();
    assert_eq!(poll_diagnostics_from_memory, poll_diagnostics);

    let stage_perf = client.browser_evaluate_json("globalThis.__FISSION_LAST_FRAME_STAGE_PERF")?;
    assert_eq!(stage_perf["renderer"], "canvas2d-software");
    assert!(stage_perf["softwareSceneRecordingMs"].is_number());
    assert!(stage_perf["softwareRasterizationMs"].is_number());
    assert!(stage_perf["softwareOutputCopyMs"].is_number());

    let submit_batch = r#"(() => {
        const bridge = globalThis.__FISSION_TEST__;
        const requests = [];
        const command = JSON.stringify({cmd:'SimulateResize',width:390,height:900});
        for (let i = 0; i < 1024; i++) requests.push(bridge.submit(command));
        const overflow = bridge.submit(command);
        const rawRejected = bridge.poll(overflow);
        return {requests, count: requests.length, overflow, transport: typeof rawRejected,
            rejected: JSON.parse(rawRejected)?.message};
    })()"#;
    let bounded = client.browser_evaluate_json(submit_batch)?;
    assert_eq!(bounded["count"], 1024);
    assert_eq!(bounded["overflow"], 0);
    assert_eq!(bounded["transport"], "string");
    assert!(bounded["rejected"]
        .as_str()
        .unwrap()
        .contains("too many pending browser test requests"));
    let requests: Vec<u32> = serde_json::from_value(bounded["requests"].clone())?;
    collect_bounded_bridge_errors(&client, &requests)?;

    // Refill every slot, not just one, to prove collection reclaimed the whole
    // bounded queue. Dispatch remains deferred and polling stays observational.
    let reused = client.browser_evaluate_json(submit_batch)?;
    assert_eq!(reused["count"], 1024);
    assert_eq!(reused["overflow"], 0);
    assert_eq!(reused["transport"], "string");
    assert!(reused["rejected"]
        .as_str()
        .unwrap()
        .contains("too many pending browser test requests"));
    let requests: Vec<u32> = serde_json::from_value(reused["requests"].clone())?;
    collect_bounded_bridge_errors(&client, &requests)?;

    assert!(client
        .get_text()?
        .iter()
        .any(|item| item.text == "Fission web smoke"));
    Ok(())
}

#[test]
#[ignore = "requires FISSION_BRIDGE_WEB_URL, the opt-in Web test bridge, and Chromium"]
fn web_bridge_readiness_recovers_after_document_reload() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_BRIDGE_WEB_URL")?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;
    let mut previous_time_origin = client
        .browser_evaluate_json("performance.timeOrigin")?
        .as_f64()
        .expect("browser time origin is numeric");

    for _ in 0..3 {
        client.browser_evaluate_json(
            "globalThis.__FISSION_RELOAD_SENTINEL__ = 'old document'; window.location.reload(); true",
        )?;
        client.wait_for_visible(SelectorQuery::label("Increment"), 30_000)?;

        let reloaded_page = client.browser_evaluate_json(
            "({timeOrigin: performance.timeOrigin, sentinel: globalThis.__FISSION_RELOAD_SENTINEL__ ?? null, bridgeReady: typeof globalThis.__FISSION_TEST__?.poll === 'function'})",
        )?;
        let time_origin = reloaded_page["timeOrigin"]
            .as_f64()
            .expect("browser time origin is numeric");
        assert_ne!(
            time_origin, previous_time_origin,
            "wait_for_visible must not succeed against the document being replaced"
        );
        assert!(reloaded_page["sentinel"].is_null());
        assert_eq!(reloaded_page["bridgeReady"], true);
        previous_time_origin = time_origin;
    }

    assert!(client
        .get_text()?
        .iter()
        .any(|item| item.text == "Fission web smoke"));
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
    assert_eq!(c.browser_evaluate_json("(() => {const d=Object.getOwnPropertyDescriptor(__FISSION_TEST__,'frame');const snapshot=JSON.parse(__FISSION_TEST__.frame);snapshot.width=-1;return {getter:typeof d.get,setter:typeof d.set,width:JSON.parse(__FISSION_TEST__.frame).width,legacy:typeof globalThis.__FISSION_REVIEW_FRAME};})()")?, json!({"getter":"function","setter":"undefined","width":1280,"legacy":"undefined"}));
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

    let bounded = c.browser_evaluate_json(
        "(() => { const bridge=globalThis.__FISSION_TEST__; const read=id=>JSON.parse(bridge.poll(id)); const requests=[]; const command=JSON.stringify({cmd:'SimulateResize',width:390,height:900}); for(let i=0;i<1024;i++) requests.push(bridge.submit(command)); const overflow=bridge.submit(command); const rawRejected=bridge.poll(overflow); const rejected=JSON.parse(rawRejected); const collected=requests.every(id=>read(id)?.status==='Error'); return {count:requests.length,overflow,transport:typeof rawRejected,rejected:rejected?.message,collected}; })()",
    )?;
    assert_eq!(bounded["count"], 1024);
    assert_eq!(bounded["overflow"], 0);
    assert_eq!(bounded["transport"], "string");
    assert!(bounded["rejected"]
        .as_str()
        .unwrap()
        .contains("too many pending browser test requests"));
    assert_eq!(bounded["collected"], true);

    // Freeze only the acknowledgement in this disposable development page.
    // Actual Chromium/canvas resize still occurs, but cannot claim success.
    c.browser_evaluate_json("globalThis.__savedFrame=Object.getOwnPropertyDescriptor(__FISSION_TEST__,'frame');Object.defineProperty(__FISSION_TEST__,'frame',{value:JSON.stringify({...JSON.parse(__FISSION_TEST__.frame),width:390}),writable:false,configurable:true});true")?;
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

    // Exercise the JSON bridge repeatedly, including the event-loop response
    // path used by semantic queries. Completed responses must be removed so a
    // long-running LiveTest does not accumulate bridge state.
    for _ in 0..128 {
        assert!(c.get_text()?.iter().any(|item| item.text == "Count: 1"));
    }

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
#[ignore = "requires FISSION_RESIZE_WEBGPU_URL, WebGPU-enabled Chromium, and loopback sockets"]
fn webgpu_renderer_resizes_without_recreating_mapped_config_buffer() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_RESIZE_WEBGPU_URL")?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(&url).fission_canvas())?;
    let report = client
        .browser_report()
        .ok_or_else(|| anyhow::anyhow!("browser report is unavailable"))?;
    assert_eq!(
        report.renderer.as_deref(),
        Some("webgpu-vello"),
        "the fixture must exercise the WebGPU renderer: {report:?}"
    );

    capture_webgpu_frame(&client, "webgpu-initial", 1280, 900)?;
    let initial = observe(&client)?;
    assert_eq!(initial["frame"]["phase"], "submitted");
    for (step, width) in [390, 800, 1280, 800, 390].into_iter().enumerate() {
        client.simulate_resize(width, 900)?;
        let resized = observe(&client)?;
        assert_eq!(resized["width"], width);
        assert_eq!(resized["height"], 900);
        assert_eq!(resized["canvas"]["pixel_width"], width);
        assert_eq!(resized["canvas"]["pixel_height"], 900);
        assert_eq!(resized["frame"]["phase"], "submitted");
        capture_webgpu_frame(&client, &format!("webgpu-step-{step}"), width, 900)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires FISSION_RESIZE_WEBGPU_URL, WebGPU-enabled Chromium, and loopback sockets"]
fn webgpu_screenshots_preserve_device_scale_two_across_host_resize() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_RESIZE_WEBGPU_URL")?;
    let mut options = BrowserTestOptions::new(&url).fission_canvas();
    options.viewport_width = 800;
    options.viewport_height = 900;
    options.device_scale_factor = 2.0;
    let client = LiveTestClient::launch_browser(options)?;
    let report = client
        .browser_report()
        .ok_or_else(|| anyhow::anyhow!("browser report is unavailable"))?;
    assert_eq!(
        report.renderer.as_deref(),
        Some("webgpu-vello"),
        "requires actual WebGPU: {report:?}"
    );
    let initial = observe(&client)?;
    assert_eq!(initial["scale"], 2);
    assert_eq!(initial["width"], 800);
    assert_eq!(initial["height"], 900);
    assert_eq!(initial["canvas"]["pixel_width"], 1600);
    assert_eq!(initial["canvas"]["pixel_height"], 1800);
    capture_webgpu_frame(&client, "webgpu-dpr2-initial", 1600, 1800)?;

    client.simulate_resize(390, 900)?;
    let resized = observe(&client)?;
    assert_eq!(resized["timeOrigin"], initial["timeOrigin"]);
    assert_eq!(resized["url"], initial["url"]);
    assert_eq!(resized["scale"], 2);
    assert_eq!(resized["width"], 390);
    assert_eq!(resized["height"], 900);
    assert_eq!(resized["canvas"]["pixel_width"], 780);
    assert_eq!(resized["canvas"]["pixel_height"], 1800);
    assert_eq!(resized["frame"]["width"], 390);
    assert_eq!(resized["frame"]["height"], 900);
    assert_eq!(resized["frame"]["phase"], "submitted");
    capture_webgpu_frame(&client, "webgpu-dpr2-narrow", 780, 1800)?;
    Ok(())
}

#[test]
#[ignore = "requires FISSION_RESIZE_WEBGPU_URL, WebGPU-enabled Chromium, and loopback sockets"]
fn webgpu_renderer_reloads_and_recreates_live_test_bridge() -> anyhow::Result<()> {
    let url = std::env::var("FISSION_RESIZE_WEBGPU_URL")?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(&url).fission_canvas())?;
    let report = client
        .browser_report()
        .ok_or_else(|| anyhow::anyhow!("browser report is unavailable"))?;
    assert_eq!(
        report.renderer.as_deref(),
        Some("webgpu-vello"),
        "the fixture must exercise the WebGPU renderer: {report:?}"
    );

    let before = client.browser_evaluate_json(
        "({timeOrigin:performance.timeOrigin,renderer:globalThis.__FISSION_RENDERER_INFO?.active,frames:globalThis.__FISSION_RENDERED_FRAME_COUNT||0})",
    )?;
    assert_eq!(before["renderer"], "webgpu-vello");
    assert!(before["frames"].as_u64().unwrap_or_default() > 0);
    capture_webgpu_frame(&client, "webgpu-before-reload", 1280, 900)?;

    client.browser_evaluate_json(
        "globalThis.__FISSION_RELOAD_SENTINEL__='old document';window.location.reload();true",
    )?;
    client.wait_for_visible(SelectorQuery::label("Increment"), 60_000)?;

    let after = client.browser_evaluate_json(
        "({timeOrigin:performance.timeOrigin,sentinel:globalThis.__FISSION_RELOAD_SENTINEL__??null,bridgeReady:typeof globalThis.__FISSION_TEST__?.poll==='function'&&typeof globalThis.__FISSION_TEST__?.submit==='function',renderer:globalThis.__FISSION_RENDERER_INFO?.active,frames:globalThis.__FISSION_RENDERED_FRAME_COUNT||0})",
    )?;
    assert_ne!(
        before["timeOrigin"], after["timeOrigin"],
        "the test must observe a committed replacement document"
    );
    assert!(after["sentinel"].is_null());
    assert_eq!(after["bridgeReady"], true);
    assert_eq!(after["renderer"], "webgpu-vello");
    assert!(after["frames"].as_u64().unwrap_or_default() > 0);
    assert!(client
        .get_text()?
        .iter()
        .any(|item| item.text == "Fission web smoke"));
    capture_webgpu_frame(&client, "webgpu-after-reload", 1280, 900)?;
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
