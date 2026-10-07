//! Exercise the public binary contract, including its owned worker and listener.
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
impl Fixture {
    fn content() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fission-preview-contract-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("platforms/site")).unwrap();
        fs::write(
            root.join("platforms/site/README.md"),
            "Static site target fixture\n",
        )
        .unwrap();
        fs::write(root.join("fission.toml"), "targets = ['static-site']\n[app]\nname = 'preview-contract'\napp_id = 'test.preview'\n").unwrap();
        fs::write(
            root.join("content/start.md"),
            "# Preview contract\n\nA real Fission content build.\n",
        )
        .unwrap();
        Self(root)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fission"));
        command
            .args([
                "preview",
                "--target",
                "static-site",
                "--json",
                "--port",
                "0",
                "--project-dir",
            ])
            .arg(&self.0)
            .env_remove("FISSION_WEB_TEST_CONTROL");
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Session {
    child: Child,
    events: Receiver<Value>,
}
impl Session {
    fn start(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let event: Value =
                    serde_json::from_str(&line.unwrap()).expect("JSON stdout contains only events");
                if sender.send(event).is_err() {
                    break;
                }
            }
        });
        Self { child, events }
    }
    fn until(&self, stage: &str) -> Value {
        loop {
            let event = self
                .events
                .recv_timeout(Duration::from_secs(30))
                .expect("bounded event wait");
            assert_eq!(event["schema"], "fission.preview.v1");
            assert_eq!(event["owner_pid"], self.child.id());
            if event["stage"] == stage {
                return event;
            }
            assert_ne!(event["stage"], "failed", "{event}");
            assert_ne!(event["stage"], "stopped", "{event}");
            assert_eq!(event["readiness"], "unverified");
            assert_eq!(event["live_test_ready"], false);
        }
    }
    fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(
                Instant::now() < deadline,
                "preview did not exit after terminal event"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn real_content_preview_reports_exact_url_and_releases_on_stop_and_eof() {
    let fixture = Fixture::content();
    let original = fs::read(fixture.0.join("fission.toml")).unwrap();
    for (mount, eof) in [("/", false), ("/repository-name/", true)] {
        let mut command = fixture.command();
        command.args(["--mount", mount, "--stdin-control"]);
        let mut session = Session::start(command);
        let ready = session.until("ready");
        assert_eq!(ready["readiness"], "local_assets");
        assert_eq!(ready["live_test_ready"], false);
        assert!(ready["detail"]["verified_local_assets"].as_u64().unwrap() >= 4);
        let url = ready["url"].as_str().unwrap();
        assert!(url.ends_with(mount));
        let address = url
            .strip_prefix("http://")
            .unwrap()
            .split('/')
            .next()
            .unwrap();
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(
                format!("GET {mount}content/start/ HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
            )
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("Preview contract"));
        if eof {
            session.child.stdin.take();
        } else {
            session
                .child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"stop\n")
                .unwrap();
        }
        let stopped = session.until("stopped");
        assert_eq!(stopped["detail"]["owned_resources_released"], true);
        session.wait();
        assert!(TcpListener::bind(address).is_ok());
    }
    assert_eq!(fs::read(fixture.0.join("fission.toml")).unwrap(), original);
}

fn failure(mut command: Command, stage: &str, detail: &str) {
    let output = command.output().unwrap();
    assert!(!output.status.success());
    let events: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(!events.iter().any(|event| event["stage"] == "ready"));
    let failed = events.last().unwrap();
    assert_eq!(failed["stage"], "failed");
    assert_eq!(failed["detail"]["failed_stage"], stage);
    assert_eq!(failed["detail"]["owned_resources_released"], true);
    assert!(
        failed["detail"]["message"]
            .as_str()
            .unwrap()
            .contains(detail),
        "{failed}"
    );
    assert!(failed["detail"]["retry_argv"].is_array());
}

#[test]
fn invalid_entry_and_occupied_port_fail_without_claiming_ready_or_touching_listener() {
    let fixture = Fixture::content();
    let mut command = fixture.command();
    command.args(["--mount", "relative"]);
    failure(command, "validating", "invalid mount");
    let mut command = fixture.command();
    command.args(["--entry", "missing.html"]);
    failure(command, "building", "missing after build");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    // Avoid duplicate --port arguments; construct the occupied-port command.
    let mut command = Command::new(env!("CARGO_BIN_EXE_fission"));
    command
        .args([
            "preview",
            "--target",
            "static-site",
            "--json",
            "--project-dir",
        ])
        .arg(&fixture.0)
        .arg("--port")
        .arg(address.port().to_string());
    failure(command, "serving", "existing listener was left untouched");
    assert!(TcpStream::connect(address).is_ok());
    assert!(TcpListener::bind(address).is_err());
}

#[test]
fn real_cargo_compile_failure_has_bounded_diagnostics_and_no_ready_event() {
    let fixture = Fixture::content();
    fs::write(fixture.0.join("fission.toml"), "targets = ['static-site']\n[app]\nname = 'preview-contract'\napp_id = 'test.preview'\n[site]\nentry = 'src/main.rs'\n").unwrap();
    fs::create_dir(fixture.0.join("src")).unwrap();
    fs::write(
        fixture.0.join("Cargo.toml"),
        "[package]\nname = 'preview-contract'\nversion = '0.0.0'\nedition = '2021'\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("src/main.rs"),
        "compile_error!(\"preview fixture compiler failure\"); fn main() {}\n",
    )
    .unwrap();
    failure(
        fixture.command(),
        "building",
        "preview fixture compiler failure",
    );
}

#[test]
fn startup_timeout_and_cancellation_stop_the_real_builder_descendant() {
    let fixture = Fixture::content();
    fs::write(fixture.0.join("fission.toml"), "targets = ['static-site']\n[app]\nname = 'preview-contract'\napp_id = 'test.preview'\n[site]\nentry = 'src/main.rs'\n").unwrap();
    fs::create_dir(fixture.0.join("src")).unwrap();
    fs::write(
        fixture.0.join("Cargo.toml"),
        "[package]\nname = 'preview-contract'\nversion = '0.0.0'\nedition = '2021'\n[workspace]\n",
    )
    .unwrap();
    fs::write(fixture.0.join("src/main.rs"), r#"
fn main() {
    if std::env::var_os("PREVIEW_FIXTURE_CHILD").is_some() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std::fs::write(std::env::var_os("PREVIEW_FIXTURE_PORT").unwrap(), listener.local_addr().unwrap().to_string()).unwrap();
        loop { std::thread::sleep(std::time::Duration::from_secs(1)); }
    }
    let _child = std::process::Command::new(std::env::current_exe().unwrap()).env("PREVIEW_FIXTURE_CHILD", "1").spawn().unwrap();
    loop { std::thread::sleep(std::time::Duration::from_secs(1)); }
}
"#).unwrap();
    // Warm only this tiny dependency-free Rust fixture so the deadline tests
    // worker termination rather than variable compiler startup time.
    assert!(Command::new("cargo")
        .args(["build", "--quiet", "--manifest-path"])
        .arg(fixture.0.join("Cargo.toml"))
        .status()
        .unwrap()
        .success());
    let port_file = fixture.0.join("owned-port");
    let unrelated = TcpListener::bind("127.0.0.1:0").unwrap();
    let unrelated_address = unrelated.local_addr().unwrap();
    for cancel in [false, true] {
        let mut command = fixture.command();
        command.env("PREVIEW_FIXTURE_PORT", &port_file);
        if cancel {
            command.arg("--stdin-control");
            let mut session = Session::start(command);
            let deadline = Instant::now() + Duration::from_secs(20);
            while !port_file.exists() {
                assert!(
                    Instant::now() < deadline,
                    "builder descendant did not start"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            session
                .child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"stop\n")
                .unwrap();
            assert_eq!(
                session.until("stopped")["detail"]["owned_resources_released"],
                true
            );
            session.wait();
        } else {
            command.args(["--startup-timeout-seconds", "2"]);
            failure(command, "building", "timed out");
        }
        let address = fs::read_to_string(&port_file).expect("owned descendant ran");
        assert!(
            TcpListener::bind(address.trim()).is_ok(),
            "owned port leaked"
        );
        fs::remove_file(&port_file).unwrap();
        assert!(TcpStream::connect(unrelated_address).is_ok());
        assert!(TcpListener::bind(unrelated_address).is_err());
    }
}

#[cfg(unix)]
#[test]
fn sigterm_stops_the_attached_preview_and_releases_its_listener() {
    let fixture = Fixture::content();
    let mut session = Session::start(fixture.command());
    let ready = session.until("ready");
    let address = ready["url"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap()
        .trim_end_matches('/');
    assert!(Command::new("kill")
        .args(["-TERM", &session.child.id().to_string()])
        .status()
        .unwrap()
        .success());
    assert_eq!(
        session.until("stopped")["detail"]["owned_resources_released"],
        true
    );
    session.wait();
    assert!(TcpListener::bind(address).is_ok());
}
