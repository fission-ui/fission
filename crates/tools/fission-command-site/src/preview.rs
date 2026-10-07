//! Owned in-process file server used by the browser preview lifecycle.

use super::{http_response, static_response};
use anyhow::{bail, Context, Result};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// A URL mount, validated before any build or listener is started.
pub fn normalize_mount(mount: &str) -> Result<String> {
    if !mount.starts_with('/')
        || mount.contains("//")
        || mount.split('/').any(|s| s == "." || s == "..")
        || !mount
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/-_.~".contains(&c))
    {
        bail!("invalid mount `{mount}`; use / or an absolute URL path such as /repository-name/");
    }
    Ok(if mount.ends_with('/') {
        mount.into()
    } else {
        format!("{mount}/")
    })
}

/// Owns the actual bound listener, so port 0 reports the OS-selected port and an
/// occupied port is never probed, reused, or terminated.
pub struct PreviewServer {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl PreviewServer {
    pub fn start(root: PathBuf, host: &str, port: u16, mount: &str, spa: bool) -> Result<Self> {
        let mount = normalize_mount(mount)?;
        let root = root
            .canonicalize()
            .context("preview output directory is missing")?;
        let listener = TcpListener::bind((host, port))
            .with_context(|| format!("cannot bind preview to {host}:{port}; retry with --port 0 or a different port; the existing listener was left untouched"))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_read_timeout(Some(Duration::from_millis(250)))?;
                        stream.set_write_timeout(Some(Duration::from_millis(250)))?;
                        // Per-request failures (including optional renderer diagnostics)
                        // do not turn into startup failures; required GETs are verified
                        // separately by the lifecycle owner.
                        if let Err(error) = respond(&mut stream, &root, &mount, spa) {
                            eprintln!("preview request: {error}");
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(())
        });
        Ok(Self {
            address,
            stop,
            handle: Some(handle),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn check_running(&mut self) -> Result<()> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            self.stop()?;
            bail!("preview server exited unexpectedly; retry the preview command");
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("preview server thread failed"))??;
        }
        Ok(())
    }
}

impl Drop for PreviewServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn respond(stream: &mut TcpStream, root: &std::path::Path, mount: &str, spa: bool) -> Result<()> {
    let mut line = String::new();
    BufReader::new(stream.try_clone()?)
        .take(8192)
        .read_line(&mut line)?;
    if line.len() >= 8192 {
        bail!("request line exceeds 8 KiB");
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("").split('?').next().unwrap_or("");
    let response = if method == "POST" && path == "/__fission/renderer" {
        // The generated renderer's diagnostic POST is best effort. No test-control
        // endpoint is installed in the file server or a static production package.
        http_response(204, "text/plain", b"", spa)
    } else if method != "GET" {
        http_response(404, "text/plain", b"not found", spa)
    } else if let Some(relative) = path.strip_prefix(mount) {
        static_response(root, &format!("/{relative}"), spa)?
    } else {
        http_response(404, "text/plain", b"outside preview mount", spa)
    };
    stream.write_all(&response)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fission-mounted-preview-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("about")).unwrap();
        std::fs::write(root.join("index.html"), "<html>Home</html>").unwrap();
        std::fs::write(root.join("about/index.html"), "<html>About</html>").unwrap();
        root
    }

    fn get(server: &PreviewServer, path: &str) -> String {
        let mut stream = TcpStream::connect(server.address()).unwrap();
        stream
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn serves_only_owned_mount_and_releases_listener_on_stop_and_drop() {
        let root = fixture_root();
        let mut server =
            PreviewServer::start(root.clone(), "127.0.0.1", 0, "/repository-name/", true).unwrap();
        let address = server.address();
        assert!(get(&server, "/repository-name/").ends_with("<html>Home</html>"));
        assert!(get(&server, "/repository-name/about/").ends_with("<html>About</html>"));
        assert!(get(&server, "/repository-name/other-route").contains("200 OK"));
        assert!(get(&server, "/repository-name/missing.wasm").contains("404 Not Found"));
        assert!(get(&server, "/repository-name-other/").contains("404 Not Found"));
        server.stop().unwrap();
        assert!(TcpListener::bind(address).is_ok());
        let dropped = PreviewServer::start(root.clone(), "127.0.0.1", 0, "/", false).unwrap();
        let address = dropped.address();
        drop(dropped);
        assert!(TcpListener::bind(address).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn occupied_port_is_an_error_and_its_listener_survives() {
        let root = fixture_root();
        let unrelated = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = unrelated.local_addr().unwrap();
        let error = PreviewServer::start(root.clone(), "127.0.0.1", address.port(), "/", false)
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("--port 0"));
        assert!(TcpStream::connect(address).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_mounts_fail_before_bind() {
        for mount in ["relative", "/../", "/a//b/", "/a?b", "/a%2fb/", "/a\\b/"] {
            assert!(normalize_mount(mount).is_err(), "{mount}");
        }
        assert_eq!(
            normalize_mount("/repository-name").unwrap(),
            "/repository-name/"
        );
    }
}
