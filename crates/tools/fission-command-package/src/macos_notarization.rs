use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use fission_command_core::MacosPackageConfig;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn notarize_macos_artifact_if_configured(
    artifact: &Path,
    macos: &MacosPackageConfig,
) -> Result<()> {
    if !macos.notarize.unwrap_or(false) {
        return Ok(());
    }

    let key_file = app_store_connect_key_file_for_notarization()?;
    let key_id = env::var("APP_STORE_CONNECT_KEY_ID")
        .context("APP_STORE_CONNECT_KEY_ID is required when macOS notarization is enabled")?;
    let issuer = env::var("APP_STORE_CONNECT_ISSUER_ID")
        .context("APP_STORE_CONNECT_ISSUER_ID is required when macOS notarization is enabled")?;
    let mut runner = run_command;
    let operation = notarize_macos_artifact_with_runner(
        artifact,
        &key_file.path,
        &key_id,
        &issuer,
        &mut runner,
    );
    let cleanup = key_file.cleanup();
    finish_with_cleanup(operation, cleanup)
}

fn notarize_macos_artifact_with_runner<F>(
    artifact: &Path,
    key_file: &Path,
    key_id: &str,
    issuer: &str,
    runner: &mut F,
) -> Result<()>
where
    F: FnMut(&str, &[OsString]) -> Result<()>,
{
    let submission = prepare_notarization_submission(artifact, runner)?;
    let operation = (|| {
        runner(
            "xcrun",
            &notarytool_arguments(submission.path(), key_file, key_id, issuer),
        )
        .context("failed to submit macOS artifact with xcrun notarytool")?;
        runner("xcrun", &stapler_arguments("staple", artifact))
            .context("failed to staple macOS notarization ticket")?;
        runner("xcrun", &stapler_arguments("validate", artifact))
            .context("failed to validate stapled macOS artifact")?;
        Ok(())
    })();
    let cleanup = submission.cleanup();
    finish_with_cleanup(operation, cleanup)
}

fn finish_with_cleanup(operation: Result<()>, cleanup: Result<()>) -> Result<()> {
    match operation {
        Err(error) => Err(error),
        Ok(()) => cleanup,
    }
}

fn prepare_notarization_submission<F>(
    artifact: &Path,
    runner: &mut F,
) -> Result<NotarizationSubmission>
where
    F: FnMut(&str, &[OsString]) -> Result<()>,
{
    match artifact
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("app") => {
            if !artifact.is_dir() {
                bail!(
                    "macOS app notarization requires an existing .app bundle directory: {}",
                    artifact.display()
                );
            }
            let temp_dir = TemporaryDirectory::create("fission-notary-app")?;
            let archive = temp_dir.path().join("notary-submission.zip");
            runner("ditto", &ditto_arguments(artifact, &archive)).with_context(|| {
                format!(
                    "failed to create temporary notarization ZIP for {}",
                    artifact.display()
                )
            })?;
            Ok(NotarizationSubmission {
                path: archive,
                temp_dir: Some(temp_dir),
            })
        }
        Some("pkg") => {
            if !artifact.is_file() {
                bail!(
                    "macOS package notarization requires an existing .pkg file: {}",
                    artifact.display()
                );
            }
            Ok(NotarizationSubmission {
                path: artifact.to_path_buf(),
                temp_dir: None,
            })
        }
        _ => bail!(
            "macOS notarization supports .app bundles and .pkg files, not {}",
            artifact.display()
        ),
    }
}

fn ditto_arguments(app_bundle: &Path, archive: &Path) -> Vec<OsString> {
    vec![
        "-c".into(),
        "-k".into(),
        "--keepParent".into(),
        app_bundle.as_os_str().to_owned(),
        archive.as_os_str().to_owned(),
    ]
}

fn notarytool_arguments(
    submission: &Path,
    key_file: &Path,
    key_id: &str,
    issuer: &str,
) -> Vec<OsString> {
    vec![
        "notarytool".into(),
        "submit".into(),
        submission.as_os_str().to_owned(),
        "--key".into(),
        key_file.as_os_str().to_owned(),
        "--key-id".into(),
        key_id.into(),
        "--issuer".into(),
        issuer.into(),
        "--wait".into(),
    ]
}

fn stapler_arguments(action: &str, artifact: &Path) -> Vec<OsString> {
    vec![
        "stapler".into(),
        action.into(),
        artifact.as_os_str().to_owned(),
    ]
}

fn run_command(program: &str, arguments: &[OsString]) -> Result<()> {
    let status = Command::new(program)
        .args(arguments)
        .status()
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} failed with {status}");
    }
    Ok(())
}

struct NotarizationSubmission {
    path: PathBuf,
    temp_dir: Option<TemporaryDirectory>,
}

impl NotarizationSubmission {
    fn path(&self) -> &Path {
        &self.path
    }

    fn cleanup(mut self) -> Result<()> {
        match self.temp_dir.take() {
            Some(temp_dir) => temp_dir.cleanup(),
            None => Ok(()),
        }
    }
}

struct TemporarySecretFile {
    path: PathBuf,
    temp_dir: Option<TemporaryDirectory>,
}

impl TemporarySecretFile {
    fn cleanup(mut self) -> Result<()> {
        match self.temp_dir.take() {
            Some(temp_dir) => temp_dir.cleanup(),
            None => Ok(()),
        }
    }
}

fn app_store_connect_key_file_for_notarization() -> Result<TemporarySecretFile> {
    if let Some(path) = env::var_os("APP_STORE_CONNECT_API_KEY_PATH") {
        return Ok(TemporarySecretFile {
            path: PathBuf::from(path),
            temp_dir: None,
        });
    }
    let key_id = env::var("APP_STORE_CONNECT_KEY_ID")
        .context("APP_STORE_CONNECT_KEY_ID is required when macOS notarization is enabled")?;
    let file_name = format!("AuthKey_{key_id}.p8");
    if let Some(raw) = env::var("APP_STORE_CONNECT_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return temporary_notary_secret_file(&file_name, raw.as_bytes());
    }
    if let Some(encoded) = env::var("APP_STORE_CONNECT_API_KEY_BASE64")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        let bytes = BASE64_STANDARD
            .decode(encoded.trim())
            .context("failed to decode APP_STORE_CONNECT_API_KEY_BASE64")?;
        return temporary_notary_secret_file(&file_name, &bytes);
    }
    bail!("APP_STORE_CONNECT_API_KEY_PATH, APP_STORE_CONNECT_API_KEY, or APP_STORE_CONNECT_API_KEY_BASE64 is required when macOS notarization is enabled")
}

fn temporary_notary_secret_file(file_name: &str, contents: &[u8]) -> Result<TemporarySecretFile> {
    let temp_dir = TemporaryDirectory::create("fission-notary-key")?;
    let path = temp_dir.path().join(file_name);
    fs::write(&path, contents).with_context(|| {
        format!(
            "failed to write temporary App Store key file {}",
            path.display()
        )
    })?;
    set_private_file_permissions(&path)?;
    Ok(TemporarySecretFile {
        path,
        temp_dir: Some(temp_dir),
    })
}

struct TemporaryDirectory {
    path: Option<PathBuf>,
}

impl TemporaryDirectory {
    fn create(prefix: &str) -> Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the Unix epoch")?
            .as_nanos();
        for _ in 0..100 {
            let counter = TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "{prefix}-{}-{timestamp}-{counter}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    let directory = Self { path: Some(path) };
                    set_private_dir_permissions(directory.path())?;
                    return Ok(directory);
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to create temporary directory {}", path.display())
                    });
                }
            }
        }
        bail!("failed to allocate a unique temporary directory for macOS notarization")
    }

    fn path(&self) -> &Path {
        self.path.as_deref().expect("temporary directory is active")
    }

    fn cleanup(mut self) -> Result<()> {
        self.remove()
    }

    fn remove(&mut self) -> Result<()> {
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        match fs::remove_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => {
                self.path = Some(path.clone());
                Err(error).with_context(|| {
                    format!("failed to remove temporary directory {}", path.display())
                })
            }
        }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_notarization_uses_temporary_ditto_zip_and_staples_original() {
        let root = TemporaryDirectory::create("fission-notary-app-test").unwrap();
        let app = root.path().join("Demo.app");
        let key = root.path().join("AuthKey_TEST.p8");
        fs::create_dir(&app).unwrap();
        fs::write(&key, "key").unwrap();
        let mut calls = Vec::<(String, Vec<OsString>)>::new();
        let mut runner = |program: &str, arguments: &[OsString]| {
            if program == "ditto" {
                fs::write(PathBuf::from(&arguments[4]), "temporary zip bytes")?;
            }
            calls.push((program.to_string(), arguments.to_vec()));
            Ok(())
        };

        notarize_macos_artifact_with_runner(
            &app,
            &key,
            "TEST",
            "00000000-0000-0000-0000-000000000000",
            &mut runner,
        )
        .unwrap();

        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0].0, "ditto");
        assert_eq!(calls[0].1[0], OsString::from("-c"));
        assert_eq!(calls[0].1[2], OsString::from("--keepParent"));
        assert_eq!(calls[0].1[3], app.as_os_str());
        let archive = PathBuf::from(&calls[1].1[2]);
        assert_eq!(
            archive.extension().and_then(|value| value.to_str()),
            Some("zip")
        );
        assert!(!archive.exists());
        assert!(!archive.parent().unwrap().exists());
        assert_eq!(calls[2].1, stapler_arguments("staple", &app));
        assert_eq!(calls[3].1, stapler_arguments("validate", &app));
        assert!(app.exists());
        root.cleanup().unwrap();
    }

    #[test]
    fn pkg_notarization_submits_package_without_temporary_archive() {
        let root = TemporaryDirectory::create("fission-notary-pkg-test").unwrap();
        let package = root.path().join("Demo.pkg");
        let key = root.path().join("AuthKey_TEST.p8");
        fs::write(&package, "package bytes").unwrap();
        fs::write(&key, "key").unwrap();
        let mut calls = Vec::<(String, Vec<OsString>)>::new();
        let mut runner = |program: &str, arguments: &[OsString]| {
            calls.push((program.to_string(), arguments.to_vec()));
            Ok(())
        };

        notarize_macos_artifact_with_runner(
            &package,
            &key,
            "TEST",
            "00000000-0000-0000-0000-000000000000",
            &mut runner,
        )
        .unwrap();

        assert_eq!(calls.len(), 3);
        assert!(calls.iter().all(|(program, _)| program == "xcrun"));
        assert_eq!(calls[0].1[2], package.as_os_str());
        assert_eq!(calls[1].1, stapler_arguments("staple", &package));
        assert_eq!(calls[2].1, stapler_arguments("validate", &package));
        assert!(package.exists());
        root.cleanup().unwrap();
    }

    #[test]
    fn app_submission_archive_is_removed_when_notarytool_fails() {
        let root = TemporaryDirectory::create("fission-notary-failure-test").unwrap();
        let app = root.path().join("Demo.app");
        let key = root.path().join("AuthKey_TEST.p8");
        fs::create_dir(&app).unwrap();
        fs::write(&key, "key").unwrap();
        let mut archive = None;
        let mut runner = |program: &str, arguments: &[OsString]| {
            if program == "ditto" {
                let path = PathBuf::from(&arguments[4]);
                fs::write(&path, "temporary zip bytes")?;
                archive = Some(path);
                return Ok(());
            }
            bail!("simulated notarytool failure")
        };

        let error = notarize_macos_artifact_with_runner(
            &app,
            &key,
            "TEST",
            "00000000-0000-0000-0000-000000000000",
            &mut runner,
        )
        .unwrap_err();
        drop(runner);

        assert!(error.to_string().contains("notarytool"));
        let archive = archive.unwrap();
        assert!(!archive.exists());
        assert!(!archive.parent().unwrap().exists());
        assert!(app.exists());
        root.cleanup().unwrap();
    }

    #[test]
    fn temporary_notary_secret_is_private_and_removed() {
        let secret = temporary_notary_secret_file("AuthKey_TEST.p8", b"notary secret").unwrap();
        let path = secret.path.clone();
        let temp_dir = path.parent().unwrap().to_path_buf();
        assert_eq!(fs::read(&path).unwrap(), b"notary secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&temp_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        secret.cleanup().unwrap();

        assert!(!path.exists());
        assert!(!temp_dir.exists());
    }
}
