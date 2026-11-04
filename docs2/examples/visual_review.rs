//! Local-only browser review of the generated documentation site.
//! Run `build --project-dir docs2` to render the site, then serve the output
//! and run `control URL PORT` to drive the browser through POST /cmd.
use anyhow::{bail, Context, Result};
use fission_test_driver::{BrowserTestOptions, LiveTestClient, SelectorQuery};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("control") {
        return fission::site::build_from_cli(fission_docs2::site_app()?);
    }
    let base_url = args.get(2).context("missing site URL")?;
    let port: u16 = args.get(3).context("missing control port")?.parse()?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let mut client = LiveTestClient::launch_browser(BrowserTestOptions::new(base_url).website())?;
    eprintln!("LiveTest control listening on 127.0.0.1:{port}");
    for incoming in listener.incoming() {
        let mut stream = incoming?;
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        let (header_end, length) = loop {
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                bail!("incomplete request");
            }
            request.extend_from_slice(&buffer[..n]);
            if let Some(index) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let header_end = index + 4;
                let headers = std::str::from_utf8(&request[..index])?;
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .context("missing Content-Length")?;
                break (header_end, length);
            }
        };
        while request.len() < header_end + length {
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                bail!("incomplete request body");
            }
            request.extend_from_slice(&buffer[..n]);
        }
        let result = (|| -> Result<Value> {
            let command: Value = serde_json::from_slice(&request[header_end..header_end + length])?;
            let mut observed = Value::Null;
            match command["cmd"].as_str().unwrap_or("") {
                "Navigate" => {
                    let path = command["path"].as_str().context("missing path")?;
                    if !path.starts_with('/') || path.starts_with("//") {
                        bail!("path must be site-relative");
                    }
                    let mut options = BrowserTestOptions::new(format!(
                        "{}{}",
                        base_url.trim_end_matches('/'),
                        path
                    ))
                    .website();
                    if let Some(width) = command["width"].as_u64() {
                        options.viewport_width = u32::try_from(width)?;
                    }
                    if let Some(height) = command["height"].as_u64() {
                        options.viewport_height = u32::try_from(height)?;
                    }
                    client = LiveTestClient::launch_browser(options)?;
                }
                "SimulateResize" => client.simulate_resize(
                    command["width"].as_u64().context("missing width")? as u32,
                    command["height"].as_u64().context("missing height")? as u32,
                )?,
                "Screenshot" => client.screenshot(
                    command["path"]
                        .as_str()
                        .context("missing screenshot path")?,
                )?,
                "Tap" => client.tap(
                    command["x"].as_f64().context("missing x")? as f32,
                    command["y"].as_f64().context("missing y")? as f32,
                )?,
                "Wheel" => {
                    let query = if let Some(identifier) = command["semantic_id"].as_str() {
                        SelectorQuery::semantic_identifier(identifier)
                    } else {
                        SelectorQuery::widget_id(
                            command["widget_id"].as_str().context("missing widget_id")?,
                        )
                    };
                    client.hover_selector(query.clone())?;
                    client.wheel_selector(
                        query,
                        command["dx"].as_f64().context("missing dx")?,
                        command["dy"].as_f64().context("missing dy")?,
                    )?
                }
                "Scroll" => {
                    client.browser_evaluate_json(&format!(
                        "(window.scrollTo(0, {}), true)",
                        command["y"].as_i64().context("missing y")?
                    ))?;
                }
                "SoftwarePreviews" => {
                    client.browser_evaluate_json(
                        "(document.querySelectorAll('iframe').forEach(frame => { const url = new URL(frame.src); url.searchParams.set('fission_renderer', 'canvas2d'); frame.src = url; }), true)",
                    )?;
                }
                "Evaluate" => {
                    observed = client.browser_evaluate_json(
                        command["script"].as_str().context("missing script")?,
                    )?;
                }
                other => bail!("unsupported review command: {other}"),
            }
            Ok(json!({"ok": true, "result": observed}))
        })();
        let body = match result {
            Ok(value) => value,
            Err(error) => json!({"ok": false, "error": format!("{error:#}")}),
        }
        .to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body)?;
    }
    Ok(())
}
