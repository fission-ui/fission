//! Shared owned file serving for run, site serve, and browser tests.

mod readiness;

use anyhow::{bail, Context, Result};
use fission_command_process::{Cancelled, CleanupError, StartupContext};
use serde::Serialize;
use std::io::{self, BufRead, Read};
use std::{
    net::{SocketAddr, TcpListener},
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
pub struct OwnedServer {
    address: SocketAddr,
    mount: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<()>>>,
}

#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub spa: bool,
    pub port_search: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 0,
            mount: "/".into(),
            spa: false,
            port_search: false,
        }
    }
}

impl OwnedServer {
    pub fn start(root: PathBuf, options: &ServerOptions) -> Result<Self> {
        let count = if options.port_search && options.port != 0 {
            50
        } else {
            0
        };
        for offset in 0..=count {
            let Some(port) = options.port.checked_add(offset) else {
                break;
            };
            match Self::start_at(
                root.clone(),
                &options.host,
                port,
                &options.mount,
                options.spa,
            ) {
                Ok(server) => {
                    if offset > 0 {
                        eprintln!(
                            "Port {}:{} is already in use; using {}:{port}.",
                            options.host, options.port, options.host
                        );
                    }
                    return Ok(server);
                }
                Err(error)
                    if offset < count
                        && error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::AddrInUse) => {}
                Err(error) => return Err(error),
            }
        }
        bail!("no available local server port")
    }

    pub fn base_url(&self) -> String {
        let address = match self.address {
            SocketAddr::V4(mut address) => {
                if address.ip().is_unspecified() {
                    address.set_ip(std::net::Ipv4Addr::LOCALHOST);
                }
                SocketAddr::V4(address)
            }
            SocketAddr::V6(mut address) => {
                if address.ip().is_unspecified() {
                    address.set_ip(std::net::Ipv6Addr::LOCALHOST);
                }
                SocketAddr::V6(address)
            }
        };
        format!("http://{address}{}", self.mount)
    }

    fn start_at(root: PathBuf, host: &str, port: u16, mount: &str, spa: bool) -> Result<Self> {
        let mount = normalize_mount(mount)?;
        let root = super::paths::absolute_root(&root)?;
        let listener = TcpListener::bind((host, port))
            .with_context(|| format!("cannot bind serving to {host}:{port}; retry with --port 0 or a different port; the existing listener was left untouched"))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_mount = mount.clone();
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // macOS can inherit O_NONBLOCK from the listener. A
                        // connection arriving before its GET must still wait
                        // for the bounded request timeout, rather than close.
                        stream.set_nonblocking(false)?;
                        stream.set_read_timeout(Some(Duration::from_millis(250)))?;
                        stream.set_write_timeout(Some(Duration::from_millis(250)))?;
                        // Per-request failures (including optional renderer diagnostics)
                        // do not turn into startup failures; required GETs are verified
                        // separately by the lifecycle owner.
                        if let Err(error) = super::handle_http_request(
                            stream.try_clone()?,
                            &root,
                            &thread_mount,
                            spa,
                        ) {
                            let idle =
                                error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                                    matches!(
                                        error.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                    )
                                });
                            if !idle {
                                eprintln!("serving request: {error}");
                            }
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
            mount,
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
            bail!("serving server exited unexpectedly; retry the serving command");
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("serving server thread failed"))??;
        }
        Ok(())
    }
}

impl Drop for OwnedServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{SystemTime, UNIX_EPOCH};
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn fixture_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fission-mounted-serving-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("about")).unwrap();
        std::fs::write(root.join("index.html"), "<html>Home</html>").unwrap();
        std::fs::write(root.join("about/index.html"), "<html>About</html>").unwrap();
        root
    }

    fn get(server: &OwnedServer, path: &str) -> String {
        let mut stream = TcpStream::connect(server.address()).unwrap();
        stream
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn waits_for_get_and_keeps_deep_route_bootstrap_at_mount_root() {
        let root = fixture_root();
        std::fs::write(
            root.join("index.html"),
            "<html><head></head><body><script src='./bootstrap.mjs'></script></body></html>",
        )
        .unwrap();
        let server =
            OwnedServer::start_at(root.clone(), "127.0.0.1", 0, "/repository-name/", true).unwrap();
        let mut stream = TcpStream::connect(server.address()).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        stream
            .write_all(b"GET /repository-name/details/nested/ HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.contains("<head><base href=\"/repository-name/\">"));
        assert!(response.contains("src='./bootstrap.mjs'"));
        assert!(!get(&server, "/repository-name/").contains("<base"));
        drop(server);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serves_only_owned_mount_and_releases_listener_on_stop_and_drop() {
        let root = fixture_root();
        let mut server =
            OwnedServer::start_at(root.clone(), "127.0.0.1", 0, "/repository-name/", true).unwrap();
        let address = server.address();
        assert!(get(&server, "/repository-name/").ends_with("<html>Home</html>"));
        assert!(get(&server, "/repository-name/about/").ends_with("<html>About</html>"));
        assert!(get(&server, "/repository-name/other-route").contains("200 OK"));
        assert!(get(&server, "/repository-name/missing.wasm").contains("404 Not Found"));
        assert!(get(&server, "/repository-name-other/").contains("404 Not Found"));
        server.stop().unwrap();
        assert!(TcpListener::bind(address).is_ok());
        let dropped = OwnedServer::start_at(root.clone(), "127.0.0.1", 0, "/", false).unwrap();
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
        let error = OwnedServer::start_at(root.clone(), "127.0.0.1", address.port(), "/", false)
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

    #[test]
    fn owned_stop_is_bounded_even_with_a_slow_request() {
        let root = fixture_root();
        let mut server = OwnedServer::start(root.clone(), &Default::default()).unwrap();
        let address = server.address();
        let mut idle = TcpStream::connect(address).unwrap();
        idle.write_all(b"GET /").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let started = std::time::Instant::now();
        server.stop().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(TcpListener::bind(address).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[derive(Clone, Debug)]
pub struct ServeOptions {
    pub mount: String,
    pub startup_timeout: Duration,
    pub stdin_control: bool,
}
impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            mount: "/".into(),
            startup_timeout: Duration::from_secs(300),
            stdin_control: false,
        }
    }
}

/// Typed facts; the CLI chooses human or JSON rendering after the same work.
#[derive(Debug, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum ServingEvent {
    Building,
    Ready {
        url: String,
        readiness: &'static str,
        verified_local_assets: usize,
    },
    Stopped {
        owned_resources_released: bool,
    },
    Failed {
        message: String,
        owned_resources_released: bool,
    },
}

pub struct ServingSession {
    pub server: OwnedServer,
    pub url: String,
    pub required_assets: usize,
}

impl ServingSession {
    pub fn start(root: PathBuf, options: &ServerOptions, startup: &StartupContext) -> Result<Self> {
        let server = OwnedServer::start(root.clone(), options)?;
        let url = server.base_url();
        let required_assets = readiness::verify(
            &root,
            "index.html",
            &url::Url::parse(&url)?,
            options.spa,
            startup.deadline(),
            || startup.cancelled(),
        )?;
        Ok(Self {
            server,
            url,
            required_assets,
        })
    }
}
pub fn serve(
    build: impl FnOnce() -> Result<std::path::PathBuf>,
    mut server: ServerOptions,
    options: ServeOptions,
    open: bool,
    mut notify: impl FnMut(ServingEvent) -> Result<()>,
) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    if options.stdin_control {
        read_stop_requests(stop.clone());
    }
    let result = (|| {
        server.mount = normalize_mount(&server.mount)?;
        let startup = StartupContext::new(options.startup_timeout, stop.clone())?;
        notify(ServingEvent::Building)?;
        let root = startup.run(build)?;
        let mut session = ServingSession::start(root, &server, &startup)?;
        notify(ServingEvent::Ready {
            url: session.url.clone(),
            readiness: "local_assets",
            verified_local_assets: session.required_assets,
        })?;
        if open {
            let _ = crate::open_url(&session.url);
        }
        while !startup.cancelled() {
            session.server.check_running()?;
            std::thread::sleep(Duration::from_millis(20));
        }
        session.server.stop()
    })();
    match result {
        Ok(()) => notify(ServingEvent::Stopped {
            owned_resources_released: true,
        }),
        Err(error)
            if error.downcast_ref::<Cancelled>().is_some()
                && error.downcast_ref::<CleanupError>().is_none() =>
        {
            notify(ServingEvent::Stopped {
                owned_resources_released: true,
            })
        }
        Err(error) => {
            notify(ServingEvent::Failed {
                message: format!("{error:#}"),
                owned_resources_released: error.downcast_ref::<CleanupError>().is_none(),
            })?;
            Err(error)
        }
    }
}

pub fn human_event(event: ServingEvent) -> Result<()> {
    match event {
        ServingEvent::Building => eprintln!("Building before starting local server..."),
        ServingEvent::Ready { url, .. } => {
            println!("Serving at {url}");
            println!("Press Ctrl+C to stop.");
        }
        ServingEvent::Stopped { .. } => eprintln!("Local server stopped."),
        ServingEvent::Failed { message, .. } => eprintln!("Local server failed: {message}"),
    }
    Ok(())
}

fn read_stop_requests(stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        loop {
            let mut line = Vec::new();
            match stdin.by_ref().take(256).read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => {
                    stop.store(true, Ordering::Release);
                    break;
                }
                Ok(_) if line == b"stop\n" || line == b"stop\r\n" => {
                    stop.store(true, Ordering::Release);
                    break;
                }
                _ => {}
            }
        }
    });
}
