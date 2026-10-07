//! Bounded commands for an attached, signal-aware CLI session.

use super::{
    termination_signals, SupervisedChild, TerminationSignals, ACTIVE_SUPERVISOR, POLL_INTERVAL,
};
use anyhow::{bail, Context, Result};
use std::{
    cell::Cell,
    collections::VecDeque,
    io::Read,
    process::{Command, Stdio},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

const LOG_LIMIT: usize = 32 * 1024;
thread_local! { static INHERITED_GROUP: Cell<bool> = const { Cell::new(false) }; }

/// Runs a CLI worker's nested commands in the process tree owned by its parent.
/// The caller must already be a child of a supervising process group/Job Object.
/// This prevents nested build commands from escaping that owner's cancellation.
pub fn in_owned_process_tree<T>(action: impl FnOnce() -> Result<T>) -> Result<T> {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            INHERITED_GROUP.set(self.0);
        }
    }
    let _reset = Reset(INHERITED_GROUP.replace(true));
    action()
}

pub(super) fn inherits_group() -> bool {
    INHERITED_GROUP.get()
}

/// One attached CLI session. Signals are installed before it starts any work.
pub struct ProcessSession {
    _active: MutexGuard<'static, ()>,
    signals: &'static TerminationSignals,
}

impl ProcessSession {
    pub fn new() -> Result<Self> {
        let active = ACTIVE_SUPERVISOR
            .lock()
            .map_err(|_| anyhow::anyhow!("process supervisor lock was poisoned"))?;
        let signals = termination_signals()?;
        signals.reset();
        Ok(Self {
            _active: active,
            signals,
        })
    }

    /// Consumes a pending Ctrl+C/SIGTERM request.
    pub fn interrupted(&self) -> bool {
        self.signals.take().is_some()
    }
}

/// Runs a worker in a fresh owned process tree, retaining only the last 32 KiB
/// from each output stream. Timeout and cancellation terminate that tree.
pub fn run_captured(
    command: &mut Command,
    label: &str,
    timeout: Duration,
    mut cancelled: impl FnMut() -> bool,
) -> Result<String> {
    if cancelled() {
        bail!("{label} cancelled before starting");
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = SupervisedChild::spawn(command).with_context(|| {
        format!("cannot start {label}; check that its required tool is installed")
    })?;
    let inner = child.child.as_mut().expect("worker is present").inner();
    let stdout = inner.stdout.take().context("missing worker stdout")?;
    let stderr = inner.stderr.take().context("missing worker stderr")?;
    let out = Arc::new(Mutex::new(VecDeque::new()));
    let err = Arc::new(Mutex::new(VecDeque::new()));
    let out_reader = capture(stdout, out.clone());
    let err_reader = capture(stderr, err.clone());
    let deadline = Instant::now() + timeout;
    let result = loop {
        if cancelled() {
            break Err(anyhow::anyhow!("{label} cancelled"));
        }
        if Instant::now() >= deadline {
            break Err(anyhow::anyhow!(
                "{label} timed out after {} seconds",
                timeout.as_secs_f64()
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(status)) => break Err(anyhow::anyhow!("{label} failed with {status}")),
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(error) => break Err(error.into()),
        }
    };
    // Also clean up descendants if the worker exited before they did.
    child
        .terminate()
        .context("failed to stop owned worker tree")?;
    out_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stdout capture failed"))?;
    err_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stderr capture failed"))?;
    let output = format!("{}{}", tail(&out), tail(&err));
    result.map_err(|error| anyhow::anyhow!("{error}\n{output}"))?;
    Ok(output)
}

fn tail(buffer: &Mutex<VecDeque<u8>>) -> String {
    let bytes: Vec<_> = buffer
        .lock()
        .expect("log buffer lock")
        .iter()
        .copied()
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn capture(
    mut reader: impl Read + Send + 'static,
    buffer: Arc<Mutex<VecDeque<u8>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut chunk = [0; 4096];
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 {
                break;
            }
            let mut tail = buffer.lock().expect("log buffer lock");
            tail.extend(&chunk[..count]);
            let excess = tail.len().saturating_sub(LOG_LIMIT);
            tail.drain(..excess);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "session::tests::capture_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("FISSION_CAPTURE_FIXTURE", mode);
        command
    }

    #[test]
    fn compile_failure_retains_bounded_actionable_log_tail() {
        let error = run_captured(
            &mut fixture("logs"),
            "compile fixture",
            Duration::from_secs(5),
            || false,
        )
        .unwrap_err();
        let detail = error.to_string();
        assert!(detail.contains("compile fixture failed"));
        assert!(detail.contains("fixture compiler: unresolved symbol"));
        assert!(!detail.contains("old log sentinel"));
        assert!(detail.len() < 2 * LOG_LIMIT + 1024);
    }

    #[test]
    fn missing_tool_and_cancellation_are_explicit() {
        let mut missing =
            Command::new(std::env::temp_dir().join("fission-tool-that-does-not-exist"));
        assert!(
            run_captured(&mut missing, "fixture tool", Duration::from_secs(1), || {
                false
            })
            .unwrap_err()
            .to_string()
            .contains("required tool is installed")
        );
        assert!(run_captured(
            &mut fixture("sleep"),
            "fixture",
            Duration::from_secs(5),
            || true
        )
        .unwrap_err()
        .to_string()
        .contains("cancelled before starting"));
    }

    #[cfg(unix)]
    fn tree_test(mode: &str, cancel: bool) {
        use std::net::TcpListener;
        let unrelated = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let unrelated_address = unrelated.local_addr().unwrap();
        let ready = std::env::temp_dir().join(format!(
            "fission-preview-process-{}-{}",
            std::process::id(),
            mode
        ));
        let _ = std::fs::remove_file(&ready);
        let mut command = fixture(mode);
        command.env("FISSION_CAPTURE_READY", &ready);
        let mut ready_since = None;
        let result = run_captured(
            &mut command,
            "owned tree fixture",
            Duration::from_secs(2),
            || {
                if ready.exists() && ready_since.is_none() {
                    ready_since = Some(Instant::now());
                }
                cancel && ready_since.is_some_and(|at| at.elapsed() > Duration::from_millis(50))
            },
        );
        if mode == "leader-exit" {
            result.unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains(if cancel {
                "cancelled"
            } else {
                "timed out"
            }));
        }
        let address: std::net::SocketAddr = std::fs::read_to_string(&ready)
            .expect("descendant must have started")
            .parse()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if TcpListener::bind(address).is_ok() {
                break;
            }
            assert!(
                Instant::now() < until,
                "owned descendant's port was not released"
            );
            std::thread::sleep(POLL_INTERVAL);
        }
        assert!(
            std::net::TcpStream::connect(unrelated_address).is_ok(),
            "unrelated listener was disturbed"
        );
        std::fs::remove_file(ready).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn timeout_releases_descendant_port_and_preserves_unrelated_listener() {
        tree_test("timeout", false);
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_releases_descendant_port() {
        tree_test("cancel", true);
    }

    #[cfg(unix)]
    #[test]
    fn worker_exit_does_not_leave_its_descendant_or_block_log_capture() {
        tree_test("leader-exit", false);
    }

    #[test]
    #[ignore]
    fn capture_fixture() {
        match std::env::var("FISSION_CAPTURE_FIXTURE").unwrap().as_str() {
            "logs" => {
                println!("old log sentinel");
                println!("{}", "x".repeat(100_000));
                eprintln!("fixture compiler: unresolved symbol");
                std::process::exit(7);
            }
            "port" => {
                let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
                std::fs::write(
                    std::env::var_os("FISSION_CAPTURE_READY").unwrap(),
                    listener.local_addr().unwrap().to_string(),
                )
                .unwrap();
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
            mode => {
                let mut child = fixture("port").spawn().unwrap();
                let ready = PathBuf::from(std::env::var_os("FISSION_CAPTURE_READY").unwrap());
                while !ready.exists() {
                    std::thread::sleep(POLL_INTERVAL);
                }
                if mode == "leader-exit" {
                    std::process::exit(0);
                }
                let _ = child.wait();
            }
        }
    }

    use std::path::PathBuf;
}
