use fission_test_driver::{
    detect_chrome, BrowserTestOptions, LiveTestClient, SelectorQuery, VisibilityState,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn response(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

fn fixture() -> anyhow::Result<(String, mpsc::Sender<()>, std::thread::JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}/", listener.local_addr()?);
    let (stop_tx, stop_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline && stop_rx.try_recv().is_err() {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap_or_default();
            let request = String::from_utf8_lossy(&request[..read]);
            let welcome = request.starts_with("GET /welcome");
            let body = if welcome {
                r#"<!doctype html><title>Welcome</title><main data-fission-node="welcome"><h1 data-fission-semantics="welcome.title">Welcome to Fission</h1><a href="/" data-fission-node="again">Sign in again</a></main>"#
            } else {
                r#"<!doctype html><title>Sign in</title><main data-fission-node="sign-in"><h1>Sign in</h1><form action="/welcome" method="get"><label>Email address<input data-fission-node="email" data-fission-semantics="sign-in.email" name="email" type="email" required></label><button data-fission-node="submit" data-fission-semantics="sign-in.submit" type="submit">Sign in</button></form></main>"#
            };
            let _ = response(&mut stream, body);
        }
    });
    Ok((url, stop_tx, handle))
}

#[test]
fn site_mode_drives_semantic_ssr_navigation() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_site())?;
    let email = SelectorQuery::semantic_identifier("sign-in.email");
    client.wait_for_visible(email.clone(), 2_000)?;
    client.fill_text_selector(email.clone(), "person@example.com")?;
    client.wait_for_value(email, "person@example.com", 2_000)?;
    client.tap_selector(SelectorQuery::role_label("button", "Sign in"))?;
    client.wait_for_idle(5_000, true)?;
    client.wait_for_text("Welcome to Fission", 5_000)?;
    client.resolve_selector(SelectorQuery::semantic_identifier("welcome.title"))?;
    assert!(client.get_tree()?.iter().any(|node| {
        node.identifier.as_deref() == Some("welcome.title")
            && node.visibility != VisibilityState::Hidden
    }));
    assert!(!client.capture_screenshot_png()?.is_empty());
    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
}
