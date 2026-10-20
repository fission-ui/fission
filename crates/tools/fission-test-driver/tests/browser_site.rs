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
            let sign_in = request.starts_with("GET /sign-in");
            let body = if welcome {
                r#"<!doctype html><title>Welcome</title><main><h1 data-testid="welcome.title">Welcome</h1><a href="/">Register again</a></main>"#
            } else if sign_in {
                r#"<!doctype html><title>Sign in</title><main><h1>Sign in instead</h1></main>"#
            } else {
                r#"<!doctype html><title>Register</title><style>html{scroll-behavior:smooth}label{display:block;height:240px}button{margin-top:120px}</style><main><h1>Register</h1><form action="/welcome" method="get"><label>Full name<input data-testid="registration.name" name="name" required></label><label>Email address<input name="email" type="email" required></label><label>Organisation<input id="registration-organisation" name="organisation" required></label><label>Role<input data-testid="registration.role" name="role" required></label><label>Country<input data-testid="registration.country" name="country" required></label><button type="submit">Create account</button><a href="/sign-in">Sign in instead</a></form></main>"#
            };
            let _ = response(&mut stream, body);
        }
    });
    Ok((url, stop_tx, handle))
}

#[test]
fn website_mode_drives_semantic_navigation_without_fission_markup() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).website())?;
    for (field, value) in [
        (SelectorQuery::test_id("registration.name"), "Alex Rivera"),
        (
            SelectorQuery::role_label("textbox", "Email address"),
            "person@example.com",
        ),
        (
            SelectorQuery::accessibility_identifier("registration-organisation"),
            "Fission Labs",
        ),
        (SelectorQuery::test_id("registration.role"), "Engineer"),
        (
            SelectorQuery::test_id("registration.country"),
            "United Kingdom",
        ),
    ] {
        client.fill_text_selector(field.clone(), value)?;
        client.wait_for_value(field, value, 2_000)?;
    }
    client.tap_selector(SelectorQuery::role_label("button", "Create account"))?;
    client.wait_for_idle(5_000, true)?;
    client.wait_for_text("Welcome", 5_000)?;
    client.resolve_selector(SelectorQuery::test_id("welcome.title"))?;
    assert!(client.get_tree()?.iter().any(|node| {
        node.identifier.as_deref() == Some("welcome.title")
            && node.visibility != VisibilityState::Hidden
    }));
    assert!(!client.capture_screenshot_png()?.is_empty());
    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
}
