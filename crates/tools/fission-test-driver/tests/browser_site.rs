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

fn redirect(stream: &mut TcpStream, location: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

fn not_found(stream: &mut TcpStream) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
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
            if request.starts_with("GET /favicon.ico") {
                let _ = not_found(&mut stream);
                continue;
            }
            let welcome = request.starts_with("GET /welcome");
            let sign_in = request.starts_with("GET /sign-in");
            let admin_login = request.starts_with("GET /admin-login");
            let disclosure = request.starts_with("GET /disclosure");
            let frame_barrier = request.starts_with("GET /frame-barrier");
            if request.starts_with("GET /protected") {
                let _ = redirect(&mut stream, "/sign-in");
                continue;
            }
            let body = if frame_barrier {
                r#"<!doctype html><title>Frame barrier</title><style>body{margin:0}.spacer{height:1600px}</style><main><div class="spacer"></div><button data-testid="save">Save changes</button></main><script>globalThis.pointerState={scrolled:false,painted:false,pressedAfterPaint:false};addEventListener('scroll',()=>{pointerState.scrolled=true;requestAnimationFrame(()=>requestAnimationFrame(()=>pointerState.painted=true));},{once:true});document.querySelector('[data-testid="save"]').addEventListener('mousedown',()=>pointerState.pressedAfterPaint=pointerState.painted);</script>"#
            } else if disclosure {
                r#"<!doctype html><title>Disclosure</title><main><h1>Account</h1><details id="credentials"><summary>Customer credentials</summary><div data-testid="credential-fields">Bucket and credential controls</div></details></main>"#
            } else if welcome {
                r#"<!doctype html><title>Welcome</title><main><h1 data-testid="welcome.title">Welcome</h1><a href="/">Register again</a></main>"#
            } else if sign_in {
                r#"<!doctype html><title>Sign in</title><main><h1>Sign in instead</h1></main>"#
            } else if admin_login {
                r#"<!doctype html><title>Admin login</title><main><h1>Admin login</h1><form><input name="username" placeholder="Username"><input name="password" type="password" placeholder="Password"><input type="submit" value="Login"></form></main>"#
            } else {
                r#"<!doctype html><title>Register</title><style>html{scroll-behavior:smooth}label{display:block;height:240px}button{margin-top:120px}</style><main><h1>Register</h1><form action="/welcome" method="get"><label>Full name<input data-testid="registration.name" name="name" required></label><label>Email address<input name="email" type="email" required></label><label>Organisation<input id="registration-organisation" name="organisation" required></label><label>Role<input data-testid="registration.role" name="role" required></label><label>Country<input data-testid="registration.country" name="country" required></label><label><input data-testid="region.eu-west" type="checkbox" name="regions" value="eu-west"> Europe West</label><button type="submit">Create account</button><a href="/sign-in">Sign in instead</a></form></main>"#
            };
            let _ = response(&mut stream, body);
        }
    });
    Ok((url, stop_tx, handle))
}

#[test]
fn website_mode_waits_for_post_scroll_frame_before_native_pointer_input() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(
        BrowserTestOptions::new(format!("{url}frame-barrier")).website(),
    )?;

    client.tap_selector(SelectorQuery::test_id("save"))?;
    let state = client.browser_evaluate_json("globalThis.pointerState")?;
    assert_eq!(state["scrolled"], true);
    assert_eq!(state["painted"], true);
    assert_eq!(state["pressedAfterPaint"], true);

    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
}

#[test]
fn website_mode_drives_native_details_disclosure() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(
        BrowserTestOptions::new(format!("{url}disclosure")).website(),
    )?;
    let summary = SelectorQuery::role_label("button", "Customer credentials");
    let content = SelectorQuery::test_id("credential-fields");

    let collapsed = client.resolve_selector(summary.clone())?;
    assert_eq!(collapsed.role, "button");
    assert_eq!(collapsed.expanded, Some(false));
    assert!(collapsed.is_sequentially_focusable());
    assert!(collapsed.actions.iter().any(|action| action == "activate"));
    assert_eq!(
        client
            .resolve_selector(content.clone().include_hidden())?
            .visibility,
        VisibilityState::Hidden
    );

    client.activate_selector(summary.clone())?;
    client.wait_for_visible(content.clone(), 2_000)?;
    assert_eq!(
        client.resolve_selector(summary.clone())?.expanded,
        Some(true)
    );

    client.activate_selector(summary.clone())?;
    assert_eq!(client.resolve_selector(summary)?.expanded, Some(false));
    assert_eq!(
        client
            .resolve_selector(content.include_hidden())?
            .visibility,
        VisibilityState::Hidden
    );

    let summary = SelectorQuery::role_label("button", "Customer credentials");
    client.focus_selector(summary.clone())?;
    client.press_key("Space", 0)?;
    assert_eq!(
        client.resolve_selector(summary.clone())?.expanded,
        Some(true)
    );
    client.press_key("Space", 0)?;
    assert_eq!(client.resolve_selector(summary)?.expanded, Some(false));

    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
}

#[test]
fn website_mode_toggles_a_below_fold_native_checkbox() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).website())?;
    let region = SelectorQuery::test_id("region.eu-west");

    let before = client.resolve_selector(region.clone().include_hidden())?;
    assert_eq!(before.checked, Some(false));
    client.scroll_into_view(region.clone())?;
    client.wait_for_visible(region.clone(), 2_000)?;
    client.toggle_selector(region.clone())?;
    let after = client.resolve_selector(region)?;
    assert_eq!(after.checked, Some(true));

    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
}

#[test]
fn website_mode_attaches_before_an_initial_redirect() -> anyhow::Result<()> {
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(
        BrowserTestOptions::new(format!("{url}protected")).website(),
    )?;

    client.wait_for_text("Sign in instead", 5_000)?;

    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
}

#[test]
fn website_mode_uses_native_input_values_and_placeholders_as_fallback_labels() -> anyhow::Result<()>
{
    if detect_chrome().is_none() {
        eprintln!("skipping browser site test because Chrome/Chromium is unavailable");
        return Ok(());
    }
    let (url, stop, server) = fixture()?;
    let client = LiveTestClient::launch_browser(
        BrowserTestOptions::new(format!("{url}admin-login")).website(),
    )?;

    client.fill_text_selector(SelectorQuery::role_label("textbox", "Username"), "admin")?;
    client.fill_text_selector(SelectorQuery::role_label("textbox", "Password"), "secret")?;
    client.resolve_selector(SelectorQuery::role_label("button", "Login"))?;

    let _ = stop.send(());
    server.join().expect("site fixture thread panicked");
    Ok(())
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
