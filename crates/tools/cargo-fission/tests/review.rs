//! Public review binary: real freshly initialized content website and Chromium.
use serde_json::Value;
use std::{
    fs,
    net::TcpListener,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fission-review-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let initialized = Command::new(env!("CARGO_BIN_EXE_fission"))
            .arg("init")
            .arg(&root)
            .arg("--name")
            .arg("review-fixture")
            .arg("--local-path")
            .arg(repo)
            .output()
            .unwrap();
        assert!(
            initialized.status.success(),
            "{}",
            String::from_utf8_lossy(&initialized.stderr)
        );
        // Read guidance before authoring anything or adding a target.
        let guidance = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(guidance.contains("Fission") && guidance.contains("review"));
        if root.join("AGENTS.fission.md").is_file() {
            assert!(!fs::read_to_string(root.join("AGENTS.fission.md"))
                .unwrap()
                .is_empty());
        }
        let target = Command::new(env!("CARGO_BIN_EXE_fission"))
            .args(["add-target", "static-site", "--project-dir"])
            .arg(&root)
            .output()
            .unwrap();
        assert!(
            target.status.success(),
            "{}",
            String::from_utf8_lossy(&target.stderr)
        );
        fs::write(
            root.join("content/start.md"),
            "# Review start\n\nA real content page.\n",
        )
        .unwrap();
        fs::write(
            root.join("content/about.md"),
            "# About review\n\nA second route.\n",
        )
        .unwrap();
        Self(root)
    }
    fn review(&self, mount: &str, extra: &[&str]) -> (i32, Value) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_fission"));
        cmd.args([
            "test",
            "--visual-review",
            "--target",
            "static-site",
            "--project-dir",
        ])
        .arg(&self.0)
        .args(["--output-dir"])
        .arg(self.0.join("review"))
        .args(["--mount", mount, "--json"])
        .args(extra)
        .env_remove("FISSION_WEB_TEST_CONTROL");
        let out = cmd.output().unwrap();
        let value: Value =
            serde_json::from_slice(&out.stdout).expect("stdout must be exactly one JSON report");
        assert_eq!(value["schema"], "fission.website-review.v1");
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(self.0.join("review/review.json")).unwrap())
                .unwrap(),
            value
        );
        (out.status.code().unwrap(), value)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
#[ignore = "requires installed Chromium and loopback sockets"]
fn fresh_content_routes_actual_matrix_mounts_artifacts_and_cleanup() {
    let f = Fixture::new();
    let config = fs::read(f.0.join("fission.toml")).unwrap();
    for mount in ["/", "/repository-name/"] {
        let (code, r) = f.review(mount, &[]);
        assert_eq!(code, 0, "{r}");
        assert_eq!(r["image_inspection_required"], true);
        assert_eq!(r["strict_warnings"], false);
        assert_eq!(r["coverage"]["source"], "static_content_metadata");
        let cases = r["cases"].as_array().unwrap();
        assert!(cases.len() >= 6);
        for case in cases {
            assert!(
                matches!(
                    case["status"].as_str(),
                    Some("clean_supported_checks" | "warning_candidates")
                ),
                "{case}"
            );
            assert_eq!(case["result"]["ready"], true);
            assert_eq!(case["result"]["viewport"][0], case["viewport"]["width"]);
            assert_eq!(case["result"]["viewport"][1], 900);
            let path =
                f.0.join("review")
                    .join(case["result"]["screenshot"].as_str().unwrap());
            let bytes = fs::read(path).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()) as u64,
                case["viewport"]["width"].as_u64().unwrap()
            );
        }
        let url = r["base_url"].as_str().unwrap();
        let authority = url
            .strip_prefix("http://")
            .unwrap()
            .split('/')
            .next()
            .unwrap();
        assert!(
            TcpListener::bind(authority).is_ok(),
            "owned preview port leaked"
        );
        assert_eq!(fs::read(f.0.join("fission.toml")).unwrap(), config);
        for path in [
            "target/fission/site/index.html",
            "target/fission/site/site-enhancement.js",
        ] {
            let source = fs::read_to_string(f.0.join(path)).unwrap();
            assert!(
                !source.contains("__FISSION_TEST__") && !source.contains("__FISSION_REVIEW_FRAME")
            );
        }
    }
    // A failing route is isolated from a successful case and cannot be clean.
    let (code, r) = f.review(
        "/",
        &[
            "--route",
            "/content/about/",
            "--route",
            "/missing/",
            "--viewport",
            "390x900",
            "--case-timeout-seconds",
            "3",
        ],
    );
    assert_eq!(code, 3, "{r}");
    assert!(r["cases"].as_array().unwrap().iter().any(|c| matches!(
        c["status"].as_str(),
        Some("clean_supported_checks" | "warning_candidates")
    )));
    assert!(r["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["status"] == "defects_detected"));
}
#[test]
fn invalid_mount_reports_execution_failure_and_preserves_unrelated_listener() {
    let f = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let (code, r) = f.review("/../escape/", &[]);
    assert_eq!(code, 1);
    assert_eq!(r["status"], "execution_failed");
    assert!(std::net::TcpStream::connect(listener.local_addr().unwrap()).is_ok());
}
#[cfg(unix)]
#[test]
fn project_and_output_symlink_spelling_is_preserved_lexically() {
    let f = Fixture::new();
    let alias = f.0.with_extension("mapped");
    std::os::unix::fs::symlink(&f.0, &alias).unwrap();
    let mapped = Fixture(alias.clone());
    let (code, report) = mapped.review("/../escape/", &[]);
    assert_eq!(code, 1);
    assert_eq!(
        PathBuf::from(report["output_dir"].as_str().unwrap()),
        alias.join("review")
    );
    assert!(alias.join("review/review.json").is_file());
}
#[test]
#[ignore = "requires installed Chromium and loopback sockets"]
fn heuristic_geometry_is_warning_only_unless_strict() {
    let f = Fixture::new();
    let cfg = f.0.join("fission.toml");
    fs::write(
        &cfg,
        format!(
            "{}\n[site]\ncss_files = [\"review.css\"]\n",
            fs::read_to_string(&cfg).unwrap()
        ),
    )
    .unwrap();
    fs::write(
        f.0.join("review.css"),
        ".fission-site-root { min-width: 1000px; }",
    )
    .unwrap();
    for strict in [false, true] {
        let mut args = vec!["--route", "/content/about/", "--viewport", "390x900"];
        if strict {
            args.push("--strict");
        }
        let (code, report) = f.review("/", &args);
        assert_eq!(code, if strict { 3 } else { 0 }, "{report}");
        assert_eq!(report["status"], "warning_candidates");
        assert_eq!(report["strict_warnings"], strict);
        assert_eq!(report["image_inspection_required"], true);
        let case = &report["cases"][0];
        assert_eq!(case["status"], "warning_candidates");
        let findings = case["result"]["findings"].as_array().unwrap();
        assert!(!findings.is_empty());
        assert!(findings.iter().all(|f| f["severity"] == "warning"));
        assert!(f
            .0
            .join("review")
            .join(case["result"]["screenshot"].as_str().unwrap())
            .is_file());
    }
}
#[cfg(unix)]
#[test]
#[ignore = "requires installed Chromium and loopback sockets"]
fn bounded_browser_timeout_and_cancel_release_owned_port() {
    let f = Fixture::new();
    let unrelated = TcpListener::bind("127.0.0.1:0").unwrap();
    let cfg = f.0.join("fission.toml");
    fs::write(
        &cfg,
        format!(
            "{}\n[site]\ncss_files = [\"review.css\"]\n",
            fs::read_to_string(&cfg).unwrap()
        ),
    )
    .unwrap();
    fs::write(f.0.join("review.css"), "body { display: none !important; }").unwrap();
    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let started = Instant::now();
    let (code, r) = f.review(
        "/",
        &[
            "--route",
            "/content/about/",
            "--viewport",
            "390x900",
            "--case-timeout-seconds",
            "1",
            "--port",
            &port.to_string(),
        ],
    );
    assert_eq!(code, 2, "{r}");
    assert!(started.elapsed() < Duration::from_secs(12));
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    // A readiness-failing browser is still active when termination arrives.
    let mut child = Command::new(env!("CARGO_BIN_EXE_fission"))
        .args([
            "test",
            "--visual-review",
            "--target",
            "static-site",
            "--project-dir",
        ])
        .arg(&f.0)
        .arg("--output-dir")
        .arg(f.0.join("cancel"))
        .args([
            "--route",
            "/content/about/",
            "--port",
            &port.to_string(),
            "--json",
        ])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Wait for this preview's actual review worker and Chromium child, so
    // cancellation covers the browser tree rather than only startup.
    let browser_pid = loop {
        let output = Command::new("ps")
            .args(["-eo", "pid,ppid,args"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        let rows: Vec<_> = text
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let pid = fields.next()?.parse::<u32>().ok()?;
                let parent = fields.next()?.parse::<u32>().ok()?;
                Some((pid, parent, fields.collect::<Vec<_>>().join(" ")))
            })
            .collect();
        if let Some(worker) = rows.iter().find(|r| {
            r.2.contains("test-visual-case") && r.2.contains(&format!("127.0.0.1:{port}"))
        }) {
            if let Some(browser) = rows.iter().find(|r| {
                r.1 == worker.0 && r.2.contains("--user-data-dir=") && r.2.contains("fission-cdp-")
            }) {
                break browser.0;
            }
        }
        assert!(Instant::now() < deadline, "owned Chromium did not launch");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let end = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    let reaped = Instant::now() + Duration::from_secs(5);
    while Command::new("kill")
        .args(["-0", &browser_pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
    {
        assert!(
            Instant::now() < reaped,
            "owned Chromium survived cancellation"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(std::net::TcpStream::connect(unrelated.local_addr().unwrap()).is_ok());
}
