#![cfg(target_os = "linux")]

// Manual native idle probe; see docs/testing/focused-input-idle.md.
use anyhow::{Context, Result};
use fission_test_driver::{LiveTestClient, SelectorQuery};
use std::{
    fs,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct OwnedApp(Child);
impl OwnedApp {
    fn wait_for_exit(&mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.0.try_wait()? {
                anyhow::ensure!(status.success(), "native fixture failed: {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                self.0.kill().context("stop unresponsive native fixture")?;
                self.0.wait().context("reap native fixture")?;
                anyhow::bail!("native fixture did not exit within {timeout:?}");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for OwnedApp {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn cpu(pid: u32) -> Result<f64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields = stat
        .rsplit_once(')')
        .context("process stat")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    // Fields 14 and 15 are process user/system ticks; the suffix starts at field 3.
    let ticks = fields.get(11).context("user CPU ticks")?.parse::<f64>()?
        + fields.get(12).context("system CPU ticks")?.parse::<f64>()?;
    let frequency = Command::new("getconf").arg("CLK_TCK").output()?;
    let hz = String::from_utf8(frequency.stdout)?.trim().parse::<f64>()?;
    Ok(ticks / hz)
}

fn sample(pid: u32, log: &PathBuf, phase: &str) -> Result<serde_json::Value> {
    thread::sleep(Duration::from_secs(2));
    let before = cpu(pid)?;
    let bytes = fs::metadata(log)?.len() as usize;
    let start = Instant::now();
    thread::sleep(Duration::from_secs(10));
    let elapsed = start.elapsed().as_secs_f64();
    let used = cpu(pid)? - before;
    let content = fs::read(log)?;
    let suffix = String::from_utf8_lossy(&content[bytes.min(content.len())..]);
    let redrawn = suffix
        .lines()
        .filter(|s| s.contains("phase=redraw_requested"))
        .count();
    let waits = suffix
        .lines()
        .filter(|s| s.contains("phase=about_to_wait"))
        .count();
    let blink = suffix.lines().filter(|s| s.contains("caret_blink")).count();
    Ok(
        serde_json::json!({"phase":phase,"elapsed_seconds":elapsed,"cpu_seconds":used,"cpu_percent_one_core":used/elapsed*100.0,"redraws":redrawn,"event_loop_waits":waits,"caret_wake_events":blink}),
    )
}
fn run(binary: &str, output: &str, blink: &str) -> Result<()> {
    anyhow::ensure!(
        matches!(blink, "0" | "1"),
        "FISSION_IDLE_BLINK must be 0 or 1"
    );
    let dir = PathBuf::from(output);
    fs::create_dir_all(&dir)?;
    let port = TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port();
    let log = dir.join(format!("idle-blink-{blink}.log"));
    let file = fs::File::create(&log)?;
    let child = Command::new(binary)
        .env("FISSION_TEST_CONTROL_PORT", port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .env("FISSION_FRAME_TRACE", "1")
        .env("FISSION_TEXTINPUT_BLINK", blink)
        .env("FISSION_TEXTINPUT_BLINK_MS", "530")
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file))
        .spawn()?;
    let mut owner = OwnedApp(child);
    let client = LiveTestClient::connect(port);
    client
        .wait_for_ready(30_000)
        .context("native renderer ready")?;
    let mut phases = vec![sample(owner.0.id(), &log, "unfocused")?];
    client.focus_selector(SelectorQuery::semantic_identifier("cpu.title"))?;
    phases.push(sample(owner.0.id(), &log, "focused_idle")?);
    client.type_text("hello")?;
    let text = client.get_text()?;
    anyhow::ensure!(
        text.iter().any(|t| t.text.contains("hello")),
        "typing visible"
    );
    client.screenshot(
        dir.join(format!("focused-blink-{blink}.png"))
            .to_str()
            .unwrap(),
    )?;
    client.focus_selector(SelectorQuery::role_label("Button", "Increment"))?;
    phases.push(sample(owner.0.id(), &log, "button_focused")?);
    for phase in [&phases[0], &phases[2]] {
        anyhow::ensure!(
            phase["redraws"].as_u64().unwrap() <= 2,
            "unfocused input kept redrawing: {phase}"
        );
    }
    let focused_redraws = phases[1]["redraws"].as_u64().unwrap();
    anyhow::ensure!(
        focused_redraws <= 25,
        "focused input redraws too often: {}",
        phases[1]
    );
    if blink == "1" {
        anyhow::ensure!(focused_redraws >= 1, "focused caret did not blink");
    } else {
        anyhow::ensure!(focused_redraws <= 2, "disabled caret kept redrawing");
    }
    client.activate_selector(SelectorQuery::role_label("Button", "Increment"))?;
    client.wait_for_text("Count: 1", 5_000)?;
    client.focus_selector(SelectorQuery::semantic_identifier("cpu.title"))?;
    client.ime_preedit("x", Some((0, 1)))?;
    client.ime_commit("é")?;
    client.wait_for_text("helloé", 5_000)?;
    client.quit()?;
    owner.wait_for_exit(Duration::from_secs(10))?;
    let report = serde_json::json!({"platform":std::env::consts::OS,"blink_enabled":blink=="1","typing_verified":true,"counter_verified":true,"ime_commit_verified":true,"phases":phases});
    fs::write(
        dir.join(format!("idle-blink-{blink}.json")),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}

#[test]
fn unresponsive_fixture_is_killed_and_reaped() -> Result<()> {
    let mut owner = OwnedApp(Command::new("sleep").arg("30").spawn()?);
    let result = owner.wait_for_exit(Duration::ZERO);
    anyhow::ensure!(result.is_err(), "unresponsive fixture must fail the probe");
    anyhow::ensure!(owner.0.try_wait()?.is_some(), "fixture must be reaped");
    Ok(())
}

#[test]
fn unsuccessful_fixture_exit_is_reported() -> Result<()> {
    let mut owner = OwnedApp(Command::new("sh").args(["-c", "exit 7"]).spawn()?);
    let error = owner
        .wait_for_exit(Duration::from_secs(5))
        .expect_err("unsuccessful fixture exit must fail the probe");
    anyhow::ensure!(error.to_string().contains("native fixture failed"));
    Ok(())
}

#[test]
#[ignore = "requires native display and FISSION_IDLE_BINARY / FISSION_IDLE_OUTPUT"]
fn native_idle_measurement() -> Result<()> {
    run(
        &std::env::var("FISSION_IDLE_BINARY")?,
        &std::env::var("FISSION_IDLE_OUTPUT")?,
        &std::env::var("FISSION_IDLE_BLINK").unwrap_or("1".into()),
    )
}
