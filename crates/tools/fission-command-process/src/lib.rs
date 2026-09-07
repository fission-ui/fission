//! Owned child-process supervision for Fission CLI commands.

use anyhow::{bail, Context, Result};
use command_group::{CommandGroup, GroupChild};
use std::process::{Command, ExitStatus};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const GRACEFUL_EXIT_TIMEOUT: Duration = Duration::from_secs(2);
static ACTIVE_SUPERVISOR: Mutex<()> = Mutex::new(());

/// A process group (Unix) or Job Object (Windows) owned by one CLI command.
pub struct SupervisedChild {
    child: Option<GroupChild>,
}

impl SupervisedChild {
    /// Spawns `command` in a new owned process group or Job Object.
    pub fn spawn(command: &mut Command) -> std::io::Result<Self> {
        command
            .group_spawn()
            .map(|child| Self { child: Some(child) })
    }

    /// Returns the operating-system identifier for the owned process group.
    pub fn id(&self) -> Option<u32> {
        self.child.as_ref().map(GroupChild::id)
    }

    /// Terminates the owned process tree and reaps its leader.
    pub fn terminate(&mut self) -> std::io::Result<Option<ExitStatus>> {
        let Some(mut child) = self.child.take() else {
            return Ok(None);
        };
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        child.kill()?;
        child.wait().map(Some)
    }

    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child
            .as_mut()
            .expect("supervised child is present while waiting")
            .try_wait()
    }

    fn forward_termination(&mut self, request: TerminationRequest) -> std::io::Result<()> {
        forward_termination(
            self.child
                .as_mut()
                .expect("supervised child is present while forwarding termination"),
            request,
        )
    }
}

impl Drop for SupervisedChild {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

/// Runs a child process under tree supervision and validates its exit status.
pub fn run_status(command: &mut Command, label: &str) -> Result<()> {
    let _active = ACTIVE_SUPERVISOR
        .lock()
        .map_err(|_| anyhow::anyhow!("process supervisor lock was poisoned"))?;
    let signals = termination_signals()?;
    signals.reset();
    let mut child =
        SupervisedChild::spawn(command).with_context(|| format!("failed to run {label}"))?;
    loop {
        if let Some(status) = child.try_wait()? {
            child.child.take();
            if !status.success() {
                bail!("{label} failed with {status}");
            }
            return Ok(());
        }
        if let Some(request) = signals.take() {
            child.forward_termination(request)?;
            let deadline = Instant::now() + GRACEFUL_EXIT_TIMEOUT;
            while Instant::now() < deadline {
                if child.try_wait()?.is_some() {
                    child.child.take();
                    bail!("{label} was interrupted");
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            child.terminate()?;
            bail!("{label} was interrupted");
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[derive(Clone, Copy)]
enum TerminationRequest {
    #[cfg(unix)]
    Interrupt,
    Terminate,
}

#[cfg(unix)]
struct TerminationSignals {
    interrupt: std::sync::Arc<std::sync::atomic::AtomicBool>,
    terminate: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(unix)]
impl TerminationSignals {
    fn reset(&self) {
        use std::sync::atomic::Ordering;
        self.interrupt.store(false, Ordering::SeqCst);
        self.terminate.store(false, Ordering::SeqCst);
    }

    fn take(&self) -> Option<TerminationRequest> {
        use std::sync::atomic::Ordering;
        if self.terminate.swap(false, Ordering::SeqCst) {
            Some(TerminationRequest::Terminate)
        } else if self.interrupt.swap(false, Ordering::SeqCst) {
            Some(TerminationRequest::Interrupt)
        } else {
            None
        }
    }
}

#[cfg(unix)]
fn termination_signals() -> Result<&'static TerminationSignals> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    static SIGNALS: OnceLock<TerminationSignals> = OnceLock::new();
    if let Some(signals) = SIGNALS.get() {
        return Ok(signals);
    }
    let signals = TerminationSignals {
        interrupt: Arc::new(AtomicBool::new(false)),
        terminate: Arc::new(AtomicBool::new(false)),
    };
    signal_hook::flag::register(SIGINT, signals.interrupt.clone())?;
    signal_hook::flag::register(SIGTERM, signals.terminate.clone())?;
    let _ = SIGNALS.set(signals);
    Ok(SIGNALS
        .get()
        .expect("termination signals are initialized before use"))
}

#[cfg(unix)]
fn forward_termination(child: &mut GroupChild, request: TerminationRequest) -> std::io::Result<()> {
    use command_group::{Signal, UnixChildExt};

    let signal = match request {
        TerminationRequest::Interrupt => Signal::SIGINT,
        TerminationRequest::Terminate => Signal::SIGTERM,
    };
    child.signal(signal)
}

#[cfg(windows)]
struct TerminationSignals {
    requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(windows)]
impl TerminationSignals {
    fn reset(&self) {
        self.requested
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn take(&self) -> Option<TerminationRequest> {
        self.requested
            .swap(false, std::sync::atomic::Ordering::SeqCst)
            .then_some(TerminationRequest::Terminate)
    }
}

#[cfg(windows)]
fn termination_signals() -> Result<&'static TerminationSignals> {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    static SIGNALS: OnceLock<TerminationSignals> = OnceLock::new();
    if let Some(signals) = SIGNALS.get() {
        return Ok(signals);
    }
    let requested = Arc::new(AtomicBool::new(false));
    let signal_requested = requested.clone();
    ctrlc::set_handler(move || {
        signal_requested.store(true, std::sync::atomic::Ordering::SeqCst);
    })?;
    let _ = SIGNALS.set(TerminationSignals { requested });
    Ok(SIGNALS
        .get()
        .expect("termination signals are initialized before use"))
}

#[cfg(windows)]
fn forward_termination(
    child: &mut GroupChild,
    _request: TerminationRequest,
) -> std::io::Result<()> {
    child.kill()
}

#[cfg(not(any(unix, windows)))]
compile_error!("fission-command-process supports Unix and Windows hosts");

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::path::PathBuf;

    #[test]
    fn propagates_successful_and_failed_exit_statuses() {
        let mut success = fixture_command(0);
        run_status(&mut success, "successful fixture").unwrap();

        let mut failure = fixture_command(7);
        let error = run_status(&mut failure, "failed fixture").unwrap_err();
        assert!(error.to_string().contains("failed fixture failed with"));
    }

    fn fixture_command(code: i32) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("tests::exit_fixture")
            .arg("--ignored")
            .env("FISSION_PROCESS_FIXTURE_EXIT", code.to_string());
        command
    }

    #[test]
    #[ignore]
    fn exit_fixture() {
        let code = std::env::var("FISSION_PROCESS_FIXTURE_EXIT")
            .unwrap()
            .parse()
            .unwrap();
        std::process::exit(code);
    }

    #[cfg(unix)]
    #[test]
    fn targeted_parent_termination_releases_owned_descendant_port() {
        use command_group::{Signal, UnixChildExt};
        use std::net::TcpListener;

        let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let ready = fixture_path("ready");
        let _ = std::fs::remove_file(&ready);
        let mut parent = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("tests::supervisor_parent_fixture")
            .arg("--ignored")
            .env("FISSION_PROCESS_FIXTURE_PORT", port.to_string())
            .env("FISSION_PROCESS_FIXTURE_READY", &ready)
            .spawn()
            .unwrap();
        if !wait_until(Duration::from_secs(5), || ready.exists()) {
            let _ = parent.kill();
            let _ = parent.wait();
            panic!("descendant did not bind its fixture port");
        }

        parent.signal(Signal::SIGTERM).unwrap();
        if !wait_until(Duration::from_secs(5), || {
            parent.try_wait().unwrap().is_some()
        }) {
            let _ = parent.kill();
            let _ = parent.wait();
            panic!("supervisor did not exit after targeted termination");
        }
        assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
        let _ = std::fs::remove_file(ready);
    }

    #[cfg(unix)]
    #[test]
    #[ignore]
    fn supervisor_parent_fixture() {
        let port = std::env::var("FISSION_PROCESS_FIXTURE_PORT").unwrap();
        let ready = std::env::var_os("FISSION_PROCESS_FIXTURE_READY").unwrap();
        let mut descendant = Command::new(std::env::current_exe().unwrap());
        descendant
            .arg("--exact")
            .arg("tests::port_holder_fixture")
            .arg("--ignored")
            .env("FISSION_PROCESS_FIXTURE_PORT", port)
            .env("FISSION_PROCESS_FIXTURE_READY", ready);

        let error = run_status(&mut descendant, "port holder fixture").unwrap_err();
        assert!(error.to_string().contains("was interrupted"));
    }

    #[cfg(unix)]
    #[test]
    #[ignore]
    fn port_holder_fixture() {
        let port = std::env::var("FISSION_PROCESS_FIXTURE_PORT")
            .unwrap()
            .parse::<u16>()
            .unwrap();
        let _listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
        std::fs::write(
            std::env::var_os("FISSION_PROCESS_FIXTURE_READY").unwrap(),
            b"ready",
        )
        .unwrap();
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    #[cfg(unix)]
    fn wait_until(timeout: Duration, mut ready: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if ready() {
                return true;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        false
    }

    #[cfg(unix)]
    fn fixture_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fission-command-process-{label}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }
}
