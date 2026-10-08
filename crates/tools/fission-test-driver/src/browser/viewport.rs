//! Shared observed CSS viewport, canvas and submitted-layout verification.
use super::*;

pub(super) fn validate_dimensions(width: u32, height: u32) -> Result<()> {
    anyhow::ensure!(
        (1..=8192).contains(&width) && (1..=8192).contains(&height)
            && u64::from(width) * u64::from(height) <= 16_777_216,
        "invalid_viewport: {width}x{height}; use 1..=8192 CSS pixels per axis and at most 16,777,216 pixels"
    );
    Ok(())
}

pub(super) fn read(client: &mut CdpClient) -> Result<Value> {
    // Read-only host observation, never shipped in production HTML.
    let result = client.send("Runtime.evaluate", json!({
        "expression": "(() => {const c=document.querySelector('canvas');const b=c?c.getBoundingClientRect():null;return {width:innerWidth,height:innerHeight,scale:devicePixelRatio,url:location.href,frame:globalThis.__FISSION_TEST__?.frame||null,canvas:b?{x:b.x,y:b.y,width:b.width,height:b.height,pixel_width:c.width,pixel_height:c.height}:null};})()",
        "returnByValue": true
    }))?;
    runtime_exception(&result)?;
    result
        .pointer("/result/value")
        .cloned()
        .context("viewport observation missing")
}

pub(super) fn acknowledged(
    v: &Value,
    o: &BrowserTestOptions,
    require_frame: bool,
    after: Option<u64>,
) -> bool {
    v["width"].as_u64() == Some(o.viewport_width as u64)
        && v["height"].as_u64() == Some(o.viewport_height as u64)
        && v["scale"].as_f64() == Some(1.0)
        && (o.mode != BrowserSmokeMode::FissionCanvas
            || (v["canvas"]["width"].as_f64() == Some(o.viewport_width as f64)
                && v["canvas"]["height"].as_f64() == Some(o.viewport_height as f64)
                && v["canvas"]["pixel_width"].as_u64() == Some(o.viewport_width as u64)
                && v["canvas"]["pixel_height"].as_u64() == Some(o.viewport_height as u64)
                && v["canvas"]["x"].as_f64() == Some(0.0)
                && v["canvas"]["y"].as_f64() == Some(0.0)
                && (!require_frame
                    || (v["frame"]["phase"].as_str() == Some("submitted")
                        && v["frame"]["width"].as_f64() == Some(o.viewport_width as f64)
                        && v["frame"]["height"].as_f64() == Some(o.viewport_height as f64)
                        && v["frame"]["frame"].as_u64().unwrap_or(0) > after.unwrap_or(0)))))
}

impl BrowserController {
    pub(super) fn resize_viewport(&mut self, width: u32, height: u32) -> Result<()> {
        validate_dimensions(width, height)?; // before any host/runtime mutation
        let deadline =
            Instant::now() + Duration::from_millis(self.options.timeout_ms.clamp(1, 60_000));
        let previous_deadline = self.client.operation_deadline.replace(deadline);
        let mut last = Value::Null;
        let result = self.resize_until(width, height, deadline, &mut last);
        self.client.operation_deadline = previous_deadline;
        // The report is observed state, including after an unsuccessful resize.
        if let (Some(w), Some(h)) = (last["width"].as_u64(), last["height"].as_u64()) {
            self.report.width = w as u32;
            self.report.height = h as u32;
        }
        result.with_context(|| format!(
            "browser_resize {width}x{height} failed; last observed {last}; metrics may already have changed. Repair the page/runtime and retry LiveTestClient::simulate_resize(width, height), or drop the client to close its owned browser"
        ))
    }

    fn resize_until(
        &mut self,
        width: u32,
        height: u32,
        deadline: Instant,
        last: &mut Value,
    ) -> Result<()> {
        *last = read(&mut self.client)?;
        let canvas = self.options.mode == BrowserSmokeMode::FissionCanvas;
        if canvas {
            let status = read_runtime_status(&mut self.client)?;
            anyhow::ensure!(status.test_bridge_ready && last["frame"]["phase"].as_str() == Some("submitted") && last["frame"]["frame"].as_u64().is_some(),
                "unsupported_host: Web canvas resize needs the matching opt-in Web test-control frame contract; compile Web with FISSION_WEB_TEST_CONTROL=1, then use LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())");
        }
        let same = last["width"].as_u64() == Some(width as u64)
            && last["height"].as_u64() == Some(height as u64)
            && acknowledged(last, &self.options, canvas, self.viewport_after)
            && self.options.viewport_width == width
            && self.options.viewport_height == height;
        let after = if same {
            None
        } else {
            last["frame"]["frame"].as_u64()
        };
        self.viewport_after = after;
        // Keep the requested state even if CDP's reply is lost: subsequent
        // capture verification cannot silently accept the previous dimensions.
        self.options.viewport_width = width;
        self.options.viewport_height = height;
        self.client
            .send(
                "Emulation.setDeviceMetricsOverride",
                json!({
                    "width":width, "height":height, "deviceScaleFactor":1, "mobile":false
                }),
            )
            .context("host_control: Chromium rejected or lost the metrics command")?;
        loop {
            *last = read(&mut self.client)?;
            self.fail_on_browser_errors()?;
            if acknowledged(last, &self.options, canvas, after) {
                self.paint_boundary()?;
                // Layout can change while waiting for presentation. Observe again.
                *last = read(&mut self.client)?;
                self.fail_on_browser_errors()?;
                if acknowledged(last, &self.options, canvas, after) {
                    return Ok(());
                }
            }
            anyhow::ensure!(Instant::now() < deadline,
                "readiness_viewport: deadline expired waiting for browser/canvas/layout frame acknowledgment");
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(25)),
            );
        }
    }

    pub(super) fn paint_boundary(&mut self) -> Result<()> {
        let result = self.client.send("Runtime.evaluate", json!({
            "expression":"new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))",
            "awaitPromise":true, "returnByValue":true
        }))?;
        runtime_exception(&result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimensions_are_bounded_without_overflow() {
        for (w, h) in [
            (0, 900),
            (390, 0),
            (8193, 1),
            (8192, 8192),
            (u32::MAX, u32::MAX),
        ] {
            assert!(validate_dimensions(w, h).is_err());
        }
        for (w, h) in [(1, 1), (8192, 2048), (1280, 900), (390, 844)] {
            assert!(validate_dimensions(w, h).is_ok());
        }
    }
    fn measured() -> Value {
        json!({"width":1280,"height":900,"scale":1,"frame":{"width":1280,"height":900,"frame":42,"phase":"submitted"},"canvas":{"width":1280,"height":900,"pixel_width":1280,"pixel_height":900,"x":0,"y":0}})
    }
    #[test]
    fn rejects_stale_frames_and_each_dimension_mismatch() {
        let o = BrowserTestOptions::new("http://localhost/").fission_canvas();
        assert!(acknowledged(&measured(), &o, true, Some(41)));
        assert!(!acknowledged(&measured(), &o, true, Some(42)));
        assert!(acknowledged(&measured(), &o, true, None)); // same-size
        for path in [
            "/width",
            "/height",
            "/scale",
            "/canvas/width",
            "/canvas/height",
            "/canvas/pixel_width",
            "/canvas/pixel_height",
            "/canvas/x",
            "/canvas/y",
            "/frame/width",
            "/frame/height",
            "/frame/frame",
            "/frame/phase",
        ] {
            let mut v = measured();
            *v.pointer_mut(path).unwrap() = if path.ends_with("/x") || path.ends_with("/y") {
                json!(1)
            } else {
                json!(0)
            };
            assert!(!acknowledged(&v, &o, true, None), "{path}");
        }
    }
    #[test]
    fn dom_requires_metrics_but_no_bridge_or_canvas_frame() {
        let o = BrowserTestOptions::new("http://localhost/");
        let mut v = json!({"width":1280,"height":900,"scale":1});
        assert!(acknowledged(&v, &o, false, None));
        v["width"] = json!(390);
        assert!(!acknowledged(&v, &o, false, None));
    }

    #[test]
    fn raw_web_resize_is_actionable_and_keeps_error_wire_shape() {
        let response = crate::unsupported_browser_resize_response();
        let TestResponse::Error { message } = response else {
            panic!("must fail");
        };
        assert!(message.contains("unsupported_host"));
        assert!(message.contains("LiveTestClient::launch_browser"));
        assert!(message.contains("client.simulate_resize(width, height)"));
        let wire = serde_json::to_value(TestResponse::Error { message }).unwrap();
        assert_eq!(wire["status"], "Error");
    }

    #[test]
    fn cdp_deadline_and_error_propagation_are_bounded() {
        for reply in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let worker = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut socket = tungstenite::accept(stream).unwrap();
                let command = socket.read().unwrap().into_text().unwrap();
                if reply {
                    let v: Value = serde_json::from_str(&command).unwrap();
                    socket
                        .send(Message::Text(
                            json!({"id":v["id"],"error":{"message":"page closed"}}).to_string(),
                        ))
                        .unwrap();
                } else {
                    std::thread::sleep(Duration::from_millis(200));
                }
            });
            let mut c = CdpClient::connect(&format!("ws://{address}")).unwrap();
            c.operation_deadline = Some(Instant::now() + Duration::from_millis(50));
            let start = Instant::now();
            let error = c
                .send("Runtime.evaluate", json!({}))
                .unwrap_err()
                .to_string();
            assert!(start.elapsed() < Duration::from_millis(180));
            assert!(
                error.contains(if reply { "page closed" } else { "timed out" })
                    || error.contains("deadline"),
                "{error}"
            );
            worker.join().unwrap();
        }
    }

    #[test]
    #[ignore = "requires FISSION_RESIZE_PRODUCTION_URL, installed Chromium, and loopback sockets"]
    fn production_canvas_has_no_test_control() -> Result<()> {
        let url = std::env::var("FISSION_RESIZE_PRODUCTION_URL")?;
        let mut controller =
            BrowserController::launch(BrowserTestOptions::new(&url).fission_canvas(), false)?;
        assert_eq!(
            controller.evaluate_json("typeof globalThis.__FISSION_TEST__")?,
            "undefined"
        );
        assert_eq!(
            controller.evaluate_json("typeof globalThis.__FISSION_REVIEW_FRAME")?,
            "undefined"
        );
        assert!(controller
            .resize_viewport(390, 900)
            .unwrap_err()
            .to_string()
            .contains("unsupported_host"));
        assert_eq!(controller.evaluate_json("innerWidth")?, 1280);
        println!(
            "production canvas rendered without test control; resize rejected before mutation"
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires FISSION_RESIZE_DOM_URL, installed Chromium, and loopback sockets"]
    fn closed_chromium_target_returns_host_failure() -> Result<()> {
        let url = std::env::var("FISSION_RESIZE_DOM_URL")?;
        let mut options = BrowserTestOptions::new(url);
        options.timeout_ms = 1_000;
        let c = crate::LiveTestClient::launch_browser(options)?;
        let crate::LiveTestTransport::Browser(controller) = &c.transport else {
            panic!("browser");
        };
        let _ = controller
            .lock()
            .unwrap()
            .client
            .send("Page.close", json!({}));
        let start = Instant::now();
        let error = c.simulate_resize(390, 900).unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(format!("{error:#}").contains("browser_resize"));
        println!("closed target: {error:#}");
        Ok(())
    }
}
