//! Finite website review through the existing Chromium driver. Each case owns
//! a fresh profile, with observation enabled before navigation.
use super::*;
use crate::{Bounds, SemanticNode};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub kind: String,
    pub severity: String,
    pub id: Option<String>,
    pub bounds: Option<Bounds>,
    pub evidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub kind: String,
    pub url: Option<String>,
    pub optional: bool,
    pub evidence: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BrowserReview {
    pub actual_url: String,
    pub ready: bool,
    pub renderer: Option<String>,
    pub rendered_frames: u64,
    pub bridge_ready: bool,
    pub viewport: Option<(u32, u32)>,
    pub frame: Option<ReviewFrame>,
    pub canvas: Option<CanvasGeometry>,
    pub screenshot: Option<PathBuf>,
    pub findings: Vec<Finding>,
    pub observations: Vec<Observation>,
    pub limitations: Vec<String>,
    pub failure: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewFrame {
    pub width: f64,
    pub height: f64,
    pub frame: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CanvasGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

/// Called in an owned, deadline-supervised review worker. No browser session
/// or arbitrary evaluation API is exposed as part of the CLI contract.
pub fn review_case(options: BrowserTestOptions, mount_url: &str) -> Result<BrowserReview> {
    let mut launch = options.clone();
    launch.url = "about:blank".into();
    let chrome = options
        .chrome_path
        .clone()
        .or_else(detect_chrome)
        .context("browser_launch: install Chromium or set FISSION_CHROME")?;
    let port = free_port();
    let session = ChromeSession::launch(&chrome, port, &launch)?;
    let ws = wait_for_target(
        port,
        "about:blank",
        Duration::from_millis(options.timeout_ms),
    )?;
    let mut client = CdpClient::connect(&ws)?;
    for domain in [
        "Page.enable",
        "Runtime.enable",
        "Log.enable",
        "Network.enable",
    ] {
        client.send(domain, json!({}))?;
    }
    client.send(
        "Emulation.setDeviceMetricsOverride",
        json!({
            "width": options.viewport_width, "height": options.viewport_height,
            "deviceScaleFactor": 1, "mobile": false
        }),
    )?;
    client.send(
        "Emulation.setEmulatedMedia",
        json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
    )?;
    client.optional_icons = vec![format!("{}/favicon.ico", url_parts(&options.url)?.0)];
    let mut controller = BrowserController {
        _session: session,
        client,
        report: BrowserSmokeReport {
            url: options.url.clone(),
            title: String::new(),
            width: 0,
            height: 0,
            renderer: None,
            body_text_len: 0,
            screenshot_path: None,
        },
    };
    let mut report = BrowserReview::default();
    let result = inspect(&mut controller, &options, mount_url, &mut report);
    if let Err(error) = result {
        report.failure = Some(bounded(&format!("{error:#}")));
    }
    let _ = controller.client.drain_events(Duration::from_millis(150));
    let icons = controller.evaluate_json("Array.from(document.querySelectorAll('link[rel]')).filter(e=>e.rel.split(/\\s+/).includes('icon')||e.rel==='apple-touch-icon').map(e=>e.href)").ok().and_then(|v| serde_json::from_value::<Vec<String>>(v).ok()).unwrap_or_default();
    report.observations = observations(&controller.client.review_events, &options.url, &icons);
    if report.observations.len() > 128 {
        report.failure.get_or_insert(
            "observation_limit: more than 128 failures; repair the reported errors and rerun"
                .into(),
        );
    }
    report.observations.truncate(128);
    if controller.client.review_events.len() >= 512 {
        report
            .limitations
            .push("Observation limit reached; case is incomplete.".into());
        report
            .failure
            .get_or_insert("observation_limit: reduce errors and rerun".into());
    }
    Ok(report)
}

fn inspect(
    c: &mut BrowserController,
    o: &BrowserTestOptions,
    mount: &str,
    r: &mut BrowserReview,
) -> Result<()> {
    c.client.send("Page.navigate", json!({"url": o.url}))?;
    let deadline = Instant::now() + Duration::from_millis(o.timeout_ms);
    loop {
        c.client.drain_events(Duration::from_millis(25))?;
        let status = read_runtime_status(&mut c.client)?;
        r.renderer = status.renderer.clone();
        r.rendered_frames = status.rendered_frames;
        r.bridge_ready = status.test_bridge_ready;
        let measured = c.evaluate_json("(() => {const c=document.querySelector('canvas');const b=c?c.getBoundingClientRect():null;return {width:innerWidth,height:innerHeight,scale:devicePixelRatio,url:location.href,frame:globalThis.__FISSION_REVIEW_FRAME||null,canvas:b?{x:b.x,y:b.y,width:b.width,height:b.height,pixel_width:c.width,pixel_height:c.height}:null};})()")?;
        r.actual_url = measured["url"].as_str().unwrap_or("").into();
        r.viewport = Some((
            measured["width"].as_u64().unwrap_or(0) as u32,
            measured["height"].as_u64().unwrap_or(0) as u32,
        ));
        r.frame = measured
            .get("frame")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());
        r.canvas = measured
            .get("canvas")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());
        let canvas = o.mode == BrowserSmokeMode::FissionCanvas;
        let acknowledged = viewport_acknowledged(&measured, o, canvas);
        if browser_is_ready(&status, o.mode, canvas) && acknowledged {
            break;
        }
        if Instant::now() >= deadline {
            anyhow::bail!("readiness_viewport: browser/layout did not acknowledge requested {}x{}; observed {measured}; use matching Fission Web shell and repair runtime errors before retrying", o.viewport_width, o.viewport_height);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let actual = url_parts(&r.actual_url)?;
    let expected = url_parts(mount)?;
    anyhow::ensure!(
        actual.0 == expected.0 && actual.1.starts_with(&expected.1),
        "navigation: route redirected outside owned mount; repair the route redirect"
    );
    r.ready = true;
    c.client.errors.clear(); // observed errors retained in review_events
    if o.mode == BrowserSmokeMode::FissionCanvas {
        ensure_response_ok(c.send_bridge_command(TestCommand::Pump {})?)?;
        let response = c.send_bridge_command(TestCommand::GetTree {})?;
        if let TestResponse::Tree { nodes } = response {
            r.findings = semantic_findings(&nodes, o.viewport_width);
            r.limitations.push("Partial: semantic bounds only; nonsemantic paint, text glyph clipping and clipping intent cannot be established. Horizontal bounds outside non-scroll ancestors are candidates for inspection. Normal vertical scrolling is not a defect.".into());
            if nodes.is_empty() {
                anyhow::bail!("semantic_tree: no semantic geometry; add semantics and rerun");
            }
        } else {
            anyhow::bail!("semantic_tree: unsupported response {response:?}");
        }
    } else {
        r.findings = dom_findings(c)?;
        r.limitations.push("Partial: horizontal DOM geometry at the initial scroll position; intentional overflow/clip ancestors are excluded. Iframes, shadow roots, pseudo-elements, text glyphs and clipping intent are not audited. Vertical document scrolling is permitted.".into());
    }
    anyhow::ensure!(
        r.findings.len() < 128,
        "geometry_limit: finding limit reached; repair the reported candidates and rerun"
    );
    let bytes = c
        .capture_page_screenshot()
        .context("capture: compositor screenshot failed; rerun with a working Chromium GPU")?;
    let decoded = image::load_from_memory(&bytes).context("capture: invalid PNG")?;
    anyhow::ensure!(
        decoded.width() == o.viewport_width && decoded.height() == o.viewport_height,
        "capture: PNG dimensions differ from actual viewport"
    );
    let pixels = decoded.to_rgba8();
    let first = pixels.get_pixel(0, 0);
    anyhow::ensure!(
        pixels.pixels().any(|p| p != first),
        "capture: uniform image has no verifiable content; repair rendering and rerun"
    );
    let path = o
        .screenshot_path
        .as_ref()
        .context("capture: output path missing")?;
    write_screenshot(path, &bytes)?;
    anyhow::ensure!(
        path.is_file() && fs::metadata(path)?.len() == bytes.len() as u64,
        "capture: screenshot file verification failed"
    );
    r.screenshot = Some(path.clone());
    Ok(())
}

fn url_parts(value: &str) -> Result<(String, String)> {
    // URL parsing stays in the command layer; Chromium returns normalized URLs.
    let (origin, path) = value.split_once("://").context("invalid browser URL")?;
    let (authority, path) = path.split_once('/').unwrap_or((path, ""));
    Ok((format!("{origin}://{authority}"), format!("/{path}")))
}
fn viewport_acknowledged(v: &Value, o: &BrowserTestOptions, canvas: bool) -> bool {
    v["width"].as_u64() == Some(o.viewport_width as u64)
        && v["height"].as_u64() == Some(o.viewport_height as u64)
        && v["scale"].as_f64() == Some(1.0)
        && (!canvas
            || (v["frame"]["width"].as_f64() == Some(o.viewport_width as f64)
                && v["frame"]["height"].as_f64() == Some(o.viewport_height as f64)
                && v["frame"]["frame"].as_u64().unwrap_or(0) > 0
                && v["canvas"]["width"].as_f64() == Some(o.viewport_width as f64)
                && v["canvas"]["height"].as_f64() == Some(o.viewport_height as f64)
                && v["canvas"]["pixel_width"].as_u64() == Some(o.viewport_width as u64)
                && v["canvas"]["pixel_height"].as_u64() == Some(o.viewport_height as u64)
                && v["canvas"]["x"].as_f64() == Some(0.0)
                && v["canvas"]["y"].as_f64() == Some(0.0)))
}

fn dom_findings(c: &mut BrowserController) -> Result<Vec<Finding>> {
    // This read-only CDP expression measures actual browser layout. It is not
    // shipped in the app or a replacement UI/interaction framework.
    let value = c.evaluate_json(r#"(() => {
      const result=[]; const all=[...document.querySelectorAll('body *')];
      if(all.length>10000) return {truncated:true, findings:[]};
      for(const e of all) {
        const s=getComputedStyle(e), b=e.getBoundingClientRect();
        if(s.display==='none'||s.visibility==='hidden'||b.width<=0||b.height<=0) continue;
        if(s.position==='fixed' && s.transform!=='none' && (b.right<=0 || b.left>=innerWidth)) continue;
        let intentional=false;
        for(let p=e.parentElement;p&&p!==document.body&&p!==document.documentElement;p=p.parentElement) {
          const ps=getComputedStyle(p);
          if(['auto','scroll','hidden','clip'].includes(ps.overflowX)) {intentional=true;break;}
        }
        if(intentional || (b.left>=-1 && b.right<=document.documentElement.clientWidth+1)) continue;
        const id=e.id ? '#'+CSS.escape(e.id) : e.hasAttribute('data-fission-node') ? '[data-fission-node="'+CSS.escape(e.getAttribute('data-fission-node'))+'"]' : (()=>{const parts=[];for(let n=e;n&&n.parentElement;n=n.parentElement){parts.unshift(n.tagName.toLowerCase()+':nth-of-type('+([...n.parentElement.children].filter(x=>x.tagName===n.tagName).indexOf(n)+1)+')');}return parts.join(' > ');})();
        result.push({kind:'horizontal_overflow',severity:'error',id,bounds:{x:b.x,y:b.y,width:b.width,height:b.height},evidence:'Browser border box exceeds horizontal viewport; no intentional overflow ancestor found.'});
        if(result.length>=128) return {truncated:true,findings:result};
      }
      return {truncated:false,findings:result};
    })()"#)?;
    anyhow::ensure!(
        value["truncated"] == false,
        "geometry_limit: DOM geometry exceeded bounded review limits; select a smaller route"
    );
    Ok(serde_json::from_value(value["findings"].clone())?)
}
fn semantic_findings(nodes: &[SemanticNode], width: u32) -> Vec<Finding> {
    let by_id: HashMap<_, _> = nodes
        .iter()
        .map(|n| (n.stable_node_id.as_str(), n))
        .collect();
    nodes.iter().filter_map(|node| {
        let b = node.logical_bounds;
        if b.width <= 0.0 || (b.x >= -1.0 && b.x+b.width <= width as f32+1.0) { return None; }
        let mut parent = node.parent.as_deref();
        for _ in 0..nodes.len() {
            let Some(p) = parent.and_then(|id| by_id.get(id)) else { break; };
            if p.scrollable_x || p.scrollable_y { return None; }
            parent = p.parent.as_deref();
        }
        if node.scrollable_x || node.scrollable_y { return None; }
        Some(Finding { kind:"horizontal_bounds_candidate".into(), severity:"warning".into(),
            id:Some(node.identifier.clone().unwrap_or_else(|| node.stable_node_id.clone())), bounds:Some(b),
            evidence:format!("Logical canvas bounds exceed viewport; visible bounds {:?}. Clipping intent is not represented; inspect screenshot.", node.visible_bounds) })
    }).take(128).collect()
}
fn bounded(s: &str) -> String {
    s.chars().take(2048).collect()
}
fn observations(events: &[Value], document: &str, icons: &[String]) -> Vec<Observation> {
    let mut requests = HashMap::new();
    let mut result = Vec::new();
    for event in events {
        let p = &event["params"];
        match event["method"].as_str().unwrap_or("") {
            "Network.requestWillBeSent" => {
                requests.insert(
                    p["requestId"].as_str().unwrap_or("").to_owned(),
                    (
                        p["request"]["url"].as_str().unwrap_or("").to_owned(),
                        p["type"].as_str().unwrap_or("").to_owned(),
                        p["request"]["method"].as_str().unwrap_or("").to_owned(),
                    ),
                );
            }
            "Network.responseReceived"
                if p["response"]["status"].as_f64().unwrap_or(0.0) >= 400.0 =>
            {
                let url = p["response"]["url"].as_str().unwrap_or("");
                result.push(network_observation(
                    url,
                    &format!("HTTP {} ({})", p["response"]["status"], p["type"]),
                    document,
                    p["type"].as_str().unwrap_or(""),
                    requests
                        .get(p["requestId"].as_str().unwrap_or(""))
                        .map(|r| r.2.as_str())
                        .unwrap_or(""),
                    icons,
                ));
            }
            "Network.loadingFailed" => {
                let url = requests
                    .get(p["requestId"].as_str().unwrap_or(""))
                    .map(|r| r.0.as_str())
                    .unwrap_or("");
                result.push(network_observation(
                    url,
                    p["errorText"].as_str().unwrap_or("resource failed"),
                    document,
                    requests
                        .get(p["requestId"].as_str().unwrap_or(""))
                        .map(|r| r.1.as_str())
                        .unwrap_or(""),
                    requests
                        .get(p["requestId"].as_str().unwrap_or(""))
                        .map(|r| r.2.as_str())
                        .unwrap_or(""),
                    icons,
                ));
            }
            "Runtime.exceptionThrown" => result.push(Observation {
                kind: "runtime_exception".into(),
                url: None,
                optional: false,
                evidence: bounded(&p["exceptionDetails"].to_string()),
            }),
            "Runtime.consoleAPICalled"
                if matches!(p["type"].as_str(), Some("error" | "assert")) =>
            {
                result.push(Observation {
                    kind: "console_error".into(),
                    url: None,
                    optional: false,
                    evidence: bounded(&p.to_string()),
                })
            }
            "Log.entryAdded" if p["entry"]["level"] == "error" => {
                let e = &p["entry"];
                let url = e["url"].as_str().unwrap_or("");
                if e["source"] == "network" {
                    let request = requests.values().find(|r| r.0 == url);
                    result.push(network_observation(
                        url,
                        e["text"].as_str().unwrap_or("network log error"),
                        document,
                        request.map(|r| r.1.as_str()).unwrap_or(""),
                        request.map(|r| r.2.as_str()).unwrap_or(""),
                        icons,
                    ));
                } else {
                    result.push(Observation {
                        kind: "browser_log".into(),
                        url: Some(url.into()),
                        optional: false,
                        evidence: bounded(&e.to_string()),
                    });
                }
            }
            _ => {}
        }
    }
    result
}
fn network_observation(
    url: &str,
    evidence: &str,
    document: &str,
    resource_type: &str,
    method: &str,
    icons: &[String],
) -> Observation {
    let origin = document
        .split_once("://")
        .and_then(|(_, p)| p.split_once('/'))
        .map(|(o, _)| format!("{}://{o}", document.split("://").next().unwrap_or("http")))
        .unwrap_or_default();
    let optional = (resource_type == "Other"
        && (url == format!("{origin}/favicon.ico")
            || (url.starts_with(&format!("{origin}/")) && icons.iter().any(|i| i == url))))
        || (method == "POST" && url == format!("{origin}/__fission/renderer"));
    Observation {
        kind: "resource_failure".into(),
        url: Some(url.into()),
        optional,
        evidence: bounded(evidence),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewport_requires_actual_browser_and_frame_acknowledgment() {
        let o = BrowserTestOptions::new("http://localhost/");
        let mut v = json!({"width":1280,"height":900,"scale":1,"frame":{"width":1280,"height":900,"frame":1},"canvas":{"width":1280,"height":900,"pixel_width":1280,"pixel_height":900,"x":0,"y":0}});
        assert!(viewport_acknowledged(&v, &o, true));
        v["width"] = json!(390);
        assert!(!viewport_acknowledged(&v, &o, true));
        v["width"] = json!(1280);
        v["frame"]["width"] = json!(390);
        assert!(!viewport_acknowledged(&v, &o, true));
        assert!(viewport_acknowledged(&v, &o, false));
    }
    #[test]
    fn resource_failures_only_allow_exact_optional_paths() {
        let doc = "http://localhost:123/repo/";
        assert!(
            network_observation(
                "http://localhost:123/favicon.ico",
                "404",
                doc,
                "Other",
                "GET",
                &[]
            )
            .optional
        );
        let icons: Vec<String> = vec!["http://localhost:123/assets/icon.png".into()];
        assert!(network_observation(&icons[0], "404", doc, "Other", "GET", &icons).optional);
        assert!(!network_observation(&icons[0], "404", doc, "Script", "GET", &icons).optional);
        assert!(
            !network_observation(
                "http://localhost:123/__fission/renderer",
                "404",
                doc,
                "Script",
                "GET",
                &icons
            )
            .optional
        );
        for p in [
            "/repo/app.wasm",
            "/repo/bootstrap.js",
            "/api/data",
            "/repo/favicon.ico",
            "/other/__fission/renderer",
        ] {
            assert!(
                !network_observation(
                    &format!("http://localhost:123{p}"),
                    "404",
                    doc,
                    "Other",
                    "GET",
                    &[]
                )
                .optional
            );
        }
    }
}
