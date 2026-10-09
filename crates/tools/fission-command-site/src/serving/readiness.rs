use anyhow::{bail, Context, Result};
use html5ever::{
    tendril::StrTendril,
    tokenizer::{BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer},
};
use regex::Regex;
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    path::Path,
    time::{Duration, Instant},
};
use url::Url;

const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_REFERENCES: usize = 2048;

#[derive(Default)]
struct Assets {
    references: Vec<String>,
    base: Option<String>,
}
impl TokenSink for Assets {
    type Handle = ();
    fn process_token(&mut self, token: Token, _line: u64) -> TokenSinkResult<()> {
        if let Token::TagToken(tag) = token {
            let attr = |name: &str| {
                tag.attrs
                    .iter()
                    .find(|a| a.name.local.as_ref() == name)
                    .map(|a| a.value.to_string())
            };
            // Favicons and document navigation are not required startup assets.
            let reference = match tag.name.as_ref() {
                "base" if self.base.is_none() => {
                    self.base = attr("href");
                    None
                }
                "script" | "img" | "source" => attr("src"),
                "meta" if attr("http-equiv").is_some_and(|v| v.eq_ignore_ascii_case("refresh")) => {
                    attr("content").and_then(|content| {
                        content.split_once(';').and_then(|(_, target)| {
                            target
                                .trim()
                                .split_once('=')
                                .filter(|(key, _)| key.trim().eq_ignore_ascii_case("url"))
                                .map(|(_, value)| {
                                    value.trim().trim_matches(['\'', '"']).to_string()
                                })
                        })
                    })
                }
                "link"
                    if attr("rel").is_some_and(|rel| {
                        rel.split_whitespace()
                            .any(|v| v == "stylesheet" || v == "modulepreload")
                    }) =>
                {
                    attr("href")
                }
                _ => None,
            };
            if let Some(reference) = reference {
                self.references.push(reference);
            }
        }
        TokenSinkResult::Continue
    }
}

/// Validates the entry response and local HTML/bootstrap/WASM dependencies
/// against the expected build output. A 200 SPA fallback cannot hide a missing
/// asset or a different entry document.
pub(super) fn verify(
    root: &Path,
    entry: &str,
    base: &Url,
    web: bool,
    deadline: Instant,
    mut cancelled: impl FnMut() -> bool,
) -> Result<usize> {
    let root = root.canonicalize()?;
    let entry_url = if entry == "index.html" {
        base.clone()
    } else {
        base.join(entry)?
    };
    let mut queue = VecDeque::from([entry_url.clone()]);
    let mut visited = BTreeSet::new();
    let quoted = Regex::new(r#"["']([^"'\s]+\.(?:wasm|m?js|css|ttf|woff2?)(?:[?#][^"']*)?)["']"#)?;
    // wasm-bindgen's import object contains module-name keys ending in .js
    // that are not files. Follow actual literal imports/URL/fetch references.
    let js_asset = Regex::new(
        r#"(?:\bfrom\s*|\bimport\s*(?:\(\s*)?|\bnew\s+URL\s*\(\s*|\bfetch\s*\(\s*)["']([^"'\s]+\.(?:wasm|m?js|css|ttf|woff2?)(?:[?#][^"']*)?)["']"#,
    )?;
    let css_url = Regex::new(r#"url\(\s*["']?([^\s'"\)]+)"#)?;
    let mut wasm_count = 0;
    let mut script_count = 0;
    while let Some(mut url) = queue.pop_front() {
        url.set_fragment(None);
        if !visited.insert(url.to_string()) {
            continue;
        }
        if visited.len() > MAX_REFERENCES {
            bail!("serving has more than {MAX_REFERENCES} required asset references");
        }
        if cancelled() {
            return Err(fission_command_process::Cancelled.into());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("serving readiness timed out")?;
        let relative = url
            .path()
            .strip_prefix(base.path())
            .context("required asset is outside the serving mount")?;
        let path = root.join(relative);
        let path = if url.path().ends_with('/') {
            path.join("index.html")
        } else {
            path
        };
        let path = path
            .canonicalize()
            .with_context(|| format!("required serving asset is missing: {url}"))?;
        if !path.starts_with(&root) || !path.is_file() {
            bail!("invalid required asset path: {url}");
        }
        if fs::metadata(&path)?.len() > MAX_ASSET_BYTES {
            bail!("required asset exceeds 64 MiB: {url}");
        }
        let expected = fs::read(&path)?;
        if expected.is_empty() {
            bail!("required asset is empty: {url}");
        }
        let (content_type, actual) = get_local_asset(&url, remaining, &mut cancelled)
            .with_context(|| format!("required serving GET failed: {url}"))?;
        if actual != expected {
            bail!("serving response differs from the expected build asset: {url}");
        }
        let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
        match extension {
            "html" => {
                if !content_type.starts_with("text/html") {
                    bail!("entry document has incorrect Content-Type: {url}");
                }
                let html =
                    std::str::from_utf8(&actual).context("entry document is not UTF-8 HTML")?;
                if !html.to_ascii_lowercase().contains("<html") {
                    bail!("entry document is not an HTML document: {url}");
                }
                let mut tokenizer = Tokenizer::new(Assets::default(), Default::default());
                let mut input = BufferQueue::default();
                input.push_back(StrTendril::from_slice(html));
                let _ = tokenizer.feed(&mut input);
                tokenizer.end();
                let document_base = tokenizer
                    .sink
                    .base
                    .as_ref()
                    .map(|reference| url.join(reference))
                    .transpose()?
                    .unwrap_or_else(|| url.clone());
                for reference in tokenizer.sink.references {
                    enqueue(&mut queue, &document_base, base, &reference)?;
                }
            }
            "js" | "mjs" | "css" => {
                if extension != "css" {
                    script_count += 1;
                    if !content_type.starts_with("text/javascript") {
                        bail!("bootstrap asset has incorrect Content-Type: {url}");
                    }
                }
                let text =
                    std::str::from_utf8(&actual).context("bootstrap or stylesheet is not UTF-8")?;
                let references = if extension == "css" {
                    &quoted
                } else {
                    &js_asset
                };
                for reference in references.captures_iter(text) {
                    enqueue(&mut queue, &url, base, &reference[1])?;
                }
                if extension == "css" {
                    for reference in css_url.captures_iter(text) {
                        enqueue(&mut queue, &url, base, &reference[1])?;
                    }
                }
            }
            "wasm" => {
                wasm_count += 1;
                if !actual.starts_with(b"\0asm\x01\0\0\0") || content_type != "application/wasm" {
                    bail!("invalid required WASM asset: {url}");
                }
            }
            _ => {}
        }
    }
    if web && (script_count == 0 || wasm_count == 0) {
        bail!("Web entry must reference its bootstrap and WASM assets; no complete bootstrap chain was found");
    }
    if Instant::now() >= deadline {
        bail!("serving readiness timed out");
    }
    Ok(visited.len())
}

// The server's internal HTTP contract is loopback/plain HTTP, Content-Length,
// and Connection: close. No redirects, DNS, TLS, or external requests are made.
fn get_local_asset(
    url: &Url,
    timeout: Duration,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(String, Vec<u8>)> {
    let ip = match url.host() {
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip),
        Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip),
        _ => bail!("asset verification requires the owned server's numeric address"),
    };
    if url.scheme() != "http" {
        bail!("asset verification requires local HTTP");
    }
    let deadline = Instant::now() + timeout;
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::new(
            ip,
            url.port_or_known_default().context("missing HTTP port")?,
        ),
        timeout.min(Duration::from_millis(250)),
    )?;
    stream.set_write_timeout(Some(timeout.min(Duration::from_millis(250))))?;
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    stream.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").as_bytes(),
    )?;
    let mut response = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        if cancelled() {
            return Err(fission_command_process::Cancelled.into());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("serving readiness timed out")?;
        stream.set_read_timeout(Some(remaining.min(Duration::from_millis(100))))?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&chunk[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue
            }
            Err(error) => return Err(error.into()),
        }
        if response.len() > MAX_ASSET_BYTES as usize + 16384 {
            bail!("asset response exceeds 64 MiB");
        }
        if response.len() > 16384 && !response.windows(4).take(16384).any(|w| w == b"\r\n\r\n") {
            bail!("asset HTTP headers exceed 16 KiB");
        }
    }
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .context("invalid asset HTTP response")?;
    if split > 16384 {
        bail!("asset HTTP headers exceed 16 KiB");
    }
    let headers = std::str::from_utf8(&response[..split])?;
    if headers
        .lines()
        .next()
        .is_none_or(|line| line.split_whitespace().nth(1) != Some("200"))
    {
        bail!("required asset did not return HTTP 200");
    }
    let header = |name: &str| {
        headers
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim())
    };
    let body = response[split + 4..].to_vec();
    if body.len() > MAX_ASSET_BYTES as usize {
        bail!("asset response exceeds 64 MiB");
    }
    let length: usize = header("Content-Length")
        .context("missing asset Content-Length")?
        .parse()?;
    if body.len() != length {
        bail!("incomplete asset response");
    }
    Ok((header("Content-Type").unwrap_or("").into(), body))
}

fn enqueue(queue: &mut VecDeque<Url>, source: &Url, base: &Url, reference: &str) -> Result<()> {
    let url = source
        .join(reference)
        .with_context(|| format!("invalid required asset reference `{reference}`"))?;
    if url.origin() != base.origin() {
        return Ok(());
    }
    if !url.path().starts_with(base.path()) {
        bail!(
            "required asset {url} is outside serving mount {}; use mount-relative asset URLs",
            base.path()
        );
    }
    if url.path().contains('%') {
        bail!("encoded required asset paths are not supported by the serving file server: {url}");
    }
    queue.push_back(url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serving::{OwnedServer, ServerOptions};
    use std::{
        net::TcpListener,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture(PathBuf);
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    impl Fixture {
        fn new(html: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "fission-serving-assets-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("pkg")).unwrap();
            fs::write(root.join("index.html"), html).unwrap();
            Self(root)
        }
        fn verify(&self, mount: &str, web: bool) -> Result<usize> {
            let server = OwnedServer::start(
                self.0.clone(),
                &ServerOptions {
                    mount: mount.into(),
                    spa: web,
                    ..Default::default()
                },
            )?;
            let base = Url::parse(&format!("http://{}{mount}", server.address()))?;
            verify(
                &self.0,
                "index.html",
                &base,
                web,
                Instant::now() + Duration::from_secs(2),
                || false,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn optional_favicon_is_not_a_required_startup_asset() {
        let fixture = Fixture::new("<!doctype html><html><head><link rel='icon' href='absent.ico'></head><body>Ready</body></html>");
        assert_eq!(fixture.verify("/", false).unwrap(), 1);
        assert_eq!(fixture.verify("/repository-name/", false).unwrap(), 1);
    }

    #[test]
    fn document_base_is_used_for_required_asset_resolution() {
        let fixture = Fixture::new(
            "<html><head><base href='/'><link rel='stylesheet' href='app.css'></head></html>",
        );
        fs::write(fixture.0.join("app.css"), "body { color: black; }").unwrap();
        assert!(fixture
            .verify("/repo/", false)
            .unwrap_err()
            .to_string()
            .contains("outside serving mount"));
        assert_eq!(fixture.verify("/", false).unwrap(), 2);
    }

    #[test]
    fn generated_root_redirect_requires_its_destination_and_assets() {
        let fixture =
            Fixture::new("<html><meta http-equiv='refresh' content='0; url=./content/'></html>");
        assert!(fixture.verify("/", false).is_err());
        fs::create_dir(fixture.0.join("content")).unwrap();
        fs::write(
            fixture.0.join("content/index.html"),
            "<html><link rel='stylesheet' href='./page.css'></html>",
        )
        .unwrap();
        assert!(fixture.verify("/repository-name/", false).is_err());
        fs::write(fixture.0.join("content/page.css"), "body { color: black; }").unwrap();
        assert_eq!(fixture.verify("/repository-name/", false).unwrap(), 3);
    }

    #[test]
    fn missing_required_bootstrap_cannot_be_hidden_by_a_spa_fallback() {
        let fixture =
            Fixture::new("<html><script type='module' src='./missing.mjs'></script></html>");
        let error = fixture.verify("/", true).unwrap_err();
        assert!(format!("{error:#}").contains("required serving asset is missing"));
    }

    #[test]
    fn verifies_bootstrap_chain_and_wasm_at_root_and_repository_mount() {
        let fixture =
            Fixture::new("<html><script type='module' src='./bootstrap.mjs'></script></html>");
        fs::write(fixture.0.join("bootstrap.mjs"), "import './pkg/app.js';").unwrap();
        fs::write(
            fixture.0.join("pkg/app.js"),
            "const imports = { './app_bg.js': {} }; const asset = new URL('app_bg.wasm', import.meta.url);",
        )
        .unwrap();
        fs::write(fixture.0.join("pkg/app_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
        assert_eq!(fixture.verify("/", true).unwrap(), 4);
        assert_eq!(fixture.verify("/repository-name/", true).unwrap(), 4);
        fs::write(fixture.0.join("pkg/app_bg.wasm"), b"wrong format").unwrap();
        assert!(fixture
            .verify("/repository-name/", true)
            .unwrap_err()
            .to_string()
            .contains("invalid required WASM"));
    }

    #[test]
    fn web_requires_a_complete_chain_and_mount_relative_assets() {
        let plain = Fixture::new("<html><body>Plain HTML</body></html>");
        assert!(plain
            .verify("/", true)
            .unwrap_err()
            .to_string()
            .contains("complete bootstrap chain"));
        let absolute = Fixture::new("<html><script src='/bootstrap.mjs'></script></html>");
        assert!(absolute
            .verify("/repository-name/", true)
            .unwrap_err()
            .to_string()
            .contains("outside serving mount"));
    }

    #[test]
    fn an_unrelated_server_cannot_supply_a_different_entry_or_fake_readiness() {
        let expected = Fixture::new("<html>Expected build</html>");
        let other = Fixture::new("<html>Unrelated listener</html>");
        let server = OwnedServer::start(other.0.clone(), &Default::default()).unwrap();
        let base = Url::parse(&format!("http://{}/", server.address())).unwrap();
        assert!(verify(
            &expected.0,
            "index.html",
            &base,
            false,
            Instant::now() + Duration::from_secs(1),
            || false
        )
        .unwrap_err()
        .to_string()
        .contains("differs from the expected build"));
    }

    #[test]
    fn bound_socket_without_http_readiness_times_out() {
        let fixture = Fixture::new("<html>Waiting</html>");
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let handle = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(300));
        });
        let started = Instant::now();
        let error = verify(
            &fixture.0,
            "index.html",
            &base,
            false,
            Instant::now() + Duration::from_millis(100),
            || false,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("required serving GET failed"));
        assert!(started.elapsed() < Duration::from_millis(250));
        handle.join().unwrap();
    }

    #[test]
    fn verification_observes_cancellation_and_deadline() {
        let fixture = Fixture::new("<html>Ready</html>");
        let base = Url::parse("http://127.0.0.1:1/").unwrap();
        assert!(verify(
            &fixture.0,
            "index.html",
            &base,
            false,
            Instant::now() + Duration::from_secs(1),
            || true
        )
        .unwrap_err()
        .to_string()
        .contains("cancelled"));
        assert!(verify(
            &fixture.0,
            "index.html",
            &base,
            false,
            Instant::now(),
            || false
        )
        .unwrap_err()
        .to_string()
        .contains("timed out"));
    }
}
