//! Bounded, credential-redacted child diagnostics for finite machine-readable commands.

use super::*;
use std::io::{Read, Write};
use std::process::Stdio;

pub const DIAGNOSTIC_LIMIT: usize = 8192;

/// Bounds and redacts a library diagnostic before exposing it in a result.
pub fn excerpt(text: &str) -> String {
    drain(
        text.as_bytes(),
        &inherited_secrets(&Command::new("fission")),
    )
}

#[derive(Debug)]
pub enum FailureKind {
    MissingExecutable,
    Exit,
    Interrupted,
    Io,
    PayloadTooLarge,
}

#[derive(Debug)]
pub struct DiagnosticFailure {
    pub kind: FailureKind,
    pub diagnostics: String,
}

/// Child stdout and stderr are drained concurrently, redacted, then sent to stderr.
/// Retains at most 4 KiB from each stream. No command debug/environment dump is emitted.
pub fn run(command: &mut Command) -> std::result::Result<(), DiagnosticFailure> {
    run_inner(command, 0, true).map(|_| ())
}

/// Captures a bounded machine payload on stdout (for Cargo metadata/artifact messages).
/// The payload is never printed. Stderr and failure excerpts use the same redaction path.
pub fn capture(
    command: &mut Command,
    limit: usize,
) -> std::result::Result<Vec<u8>, DiagnosticFailure> {
    run_inner(command, limit, true)
}

/// Reads configuration metadata without exposing parser source snippets on stderr.
pub fn capture_quiet(
    command: &mut Command,
    limit: usize,
) -> std::result::Result<Vec<u8>, DiagnosticFailure> {
    run_inner(command, limit, false)
}

fn run_inner(
    command: &mut Command,
    limit: usize,
    print: bool,
) -> std::result::Result<Vec<u8>, DiagnosticFailure> {
    let failure = |kind| DiagnosticFailure {
        kind,
        diagnostics: String::new(),
    };
    let _active = ACTIVE_SUPERVISOR
        .lock()
        .map_err(|_| failure(FailureKind::Io))?;
    let signals = termination_signals().map_err(|_| failure(FailureKind::Io))?;
    signals.reset();
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = SupervisedChild::spawn(command).map_err(|error| {
        failure(if error.kind() == std::io::ErrorKind::NotFound {
            FailureKind::MissingExecutable
        } else {
            FailureKind::Io
        })
    })?;
    let inner = child.child.as_mut().expect("spawned child").inner();
    let stdout = inner.stdout.take().expect("piped stdout");
    let stderr = inner.stderr.take().expect("piped stderr");
    let secrets = inherited_secrets(command);
    let out_secrets = secrets.clone();
    let out = std::thread::spawn(move || {
        let mut reader = PayloadReader {
            reader: stdout,
            bytes: Vec::new(),
            limit,
            overflow: false,
        };
        let tail = drain(&mut reader, &out_secrets);
        (tail, reader.bytes, reader.overflow)
    });
    let err = std::thread::spawn(move || drain(stderr, &secrets));
    let kind = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                child.child.take();
                break if status.success() {
                    None
                } else {
                    Some(FailureKind::Exit)
                };
            }
            Err(_) => {
                let _ = child.terminate();
                break Some(FailureKind::Io);
            }
            Ok(None) => {}
        }
        if let Some(request) = signals.take() {
            let _ = child.forward_termination(request);
            let deadline = Instant::now() + GRACEFUL_EXIT_TIMEOUT;
            while Instant::now() < deadline {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            let _ = child.terminate();
            break Some(FailureKind::Interrupted);
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    let (out, payload, overflow) = out.join().map_err(|_| failure(FailureKind::Io))?;
    let err = err.join().map_err(|_| failure(FailureKind::Io))?;
    // Machine payloads contain source spans/URLs; only rendered stderr is diagnostic.
    let diagnostics = if limit == 0 {
        format!("{out}{err}")
    } else {
        err.clone()
    };
    let printed = if limit == 0 { &diagnostics } else { &err };
    if print {
        let _ = std::io::stderr().write_all(printed.as_bytes());
    }
    match kind {
        None if !overflow => Ok(payload),
        None => Err(failure(FailureKind::PayloadTooLarge)),
        Some(kind) => Err(DiagnosticFailure { kind, diagnostics }),
    }
}

struct PayloadReader<R> {
    reader: R,
    bytes: Vec<u8>,
    limit: usize,
    overflow: bool,
}

impl<R: Read> Read for PayloadReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(buffer)?;
        if self.limit > 0 {
            let retain = count.min(self.limit.saturating_sub(self.bytes.len()));
            self.bytes.extend_from_slice(&buffer[..retain]);
            self.overflow |= retain < count;
        }
        Ok(count)
    }
}

fn inherited_secrets(command: &Command) -> Vec<Vec<u8>> {
    let mut values = std::env::vars_os().collect::<Vec<_>>();
    values.extend(
        command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned()))),
    );
    let mut secrets = values
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.to_string_lossy().to_ascii_uppercase();
            let sensitive = [
                "SECRET",
                "TOKEN",
                "PASSWORD",
                "PASSWD",
                "CREDENTIAL",
                "PRIVATE",
                "API_KEY",
                "ACCESS_KEY",
                "AUTH",
            ]
            .iter()
            .any(|needle| key.contains(needle));
            let bytes = value.to_string_lossy().as_bytes().to_vec();
            (sensitive && !bytes.is_empty()).then_some(bytes)
        })
        .collect::<Vec<_>>();
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    secrets.dedup();
    secrets
}

fn drain(mut reader: impl Read, secrets: &[Vec<u8>]) -> String {
    let overlap = secrets.iter().map(Vec::len).max().unwrap_or(1);
    let mut pending = Vec::new();
    let mut tail = Vec::new();
    let mut truncated = false;
    let mut buffer = [0; 4096];
    loop {
        let count = reader.read(&mut buffer).unwrap_or(0);
        pending.extend_from_slice(&buffer[..count]);
        let safe = if count == 0 {
            pending.len()
        } else {
            pending.len().saturating_sub(overlap)
        };
        let mut consumed = 0;
        while consumed < safe {
            if let Some(secret) = secrets
                .iter()
                .find(|secret| pending[consumed..].starts_with(secret))
            {
                tail.extend_from_slice(b"[REDACTED]");
                consumed += secret.len();
            } else {
                tail.push(pending[consumed]);
                consumed += 1;
            }
        }
        pending.drain(..consumed);
        if tail.len() > DIAGNOSTIC_LIMIT / 2 {
            tail.drain(..tail.len() - DIAGNOSTIC_LIMIT / 2);
            truncated = true;
        }
        if count == 0 {
            break;
        }
    }
    // Lossy UTF-8 can expand a split character; bound the final encoded excerpt too.
    let mut text = String::from_utf8_lossy(&tail).into_owned();
    if truncated {
        text = text
            .find('\n')
            .map(|index| text[index + 1..].to_string())
            .unwrap_or_default();
    }
    while text.len() > DIAGNOSTIC_LIMIT / 2 {
        text.remove(0);
    }
    let mut sanitized: String = text
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            let source_line = line
                .trim_start()
                .split_once('|')
                .is_some_and(|(prefix, _)| prefix.trim().parse::<u32>().is_ok());
            let sensitive_line = [
                "password",
                "passwd",
                "credential",
                "api_key",
                "private_key",
                "access_key",
            ]
            .iter()
            .any(|word| lower.contains(word))
                || (lower.contains("://") && lower.contains('@'));
            if source_line || sensitive_line {
                "[REDACTED diagnostic line]\n".to_string()
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    while sanitized.len() > DIAGNOSTIC_LIMIT / 2 {
        sanitized.remove(0);
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_across_reads_before_bounding_long_diagnostics() {
        let secret = b"private-credential-value".to_vec();
        let mut bytes = vec![b'x'; 4090];
        bytes.extend(&secret);
        bytes.extend(vec![b'y'; 40_000]);
        bytes.push(b'\n');
        bytes.extend(&secret);
        let text = drain(bytes.as_slice(), &[secret]);
        assert!(text.len() <= DIAGNOSTIC_LIMIT / 2);
        assert!(text.ends_with("[REDACTED]\n"));
        assert!(!text.contains("credential"));
    }

    #[test]
    fn supervised_process_failure_is_bounded_and_redacts_explicit_credentials() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "diagnostic::tests::noisy_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("FISSION_TEST_API_TOKEN", "secret-crosses-pipe-reads");
        let error = run(&mut command).unwrap_err();
        assert!(matches!(error.kind, FailureKind::Exit));
        assert!(error.diagnostics.len() <= DIAGNOSTIC_LIMIT);
        assert!(error.diagnostics.contains("[REDACTED]"));
        assert!(!error.diagnostics.contains("secret-crosses-pipe-reads"));
        let mut absent = Command::new("fission-absent-executable-fixture");
        assert!(matches!(
            run(&mut absent).unwrap_err().kind,
            FailureKind::MissingExecutable
        ));
    }

    #[test]
    fn machine_payload_overflow_is_typed_without_unbounded_capture() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "diagnostic::tests::noisy_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("FISSION_TEST_API_TOKEN", "secret")
            .env("FISSION_TEST_SUCCESS", "1");
        assert!(matches!(
            capture_quiet(&mut command, 32).unwrap_err().kind,
            FailureKind::PayloadTooLarge
        ));
    }

    #[test]
    #[ignore]
    fn noisy_fixture() {
        println!("{}", "noise\n".repeat(10_000));
        eprintln!("{}", "diagnostic\n".repeat(10_000));
        let secret = std::env::var("FISSION_TEST_API_TOKEN").unwrap();
        println!("{secret}");
        eprintln!("{secret}");
        std::process::exit(if std::env::var_os("FISSION_TEST_SUCCESS").is_some() {
            0
        } else {
            7
        });
    }
}
