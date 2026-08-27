//! Discovery and lifecycle observation for a running Fission Developer session.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{LiveTestClient, SelectorQuery};

pub const DEVELOPER_SESSION_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeveloperSessionStatus {
    Starting,
    Compiling,
    Ready,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeveloperSessionDescriptor {
    pub protocol_version: u32,
    pub session_id: String,
    pub project_manifest: PathBuf,
    pub process_id: u32,
    pub control_port: u16,
    pub bearer_token: String,
    pub application_scope: String,
    pub status: DeveloperSessionStatus,
    pub active_generation: u64,
    pub candidate_generation: Option<u64>,
    pub diagnostic: Option<String>,
}

impl DeveloperSessionDescriptor {
    pub fn write_atomic(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("developer session path has no parent"))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        let temporary = path.with_extension(format!("{}.tmp", self.process_id));
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        restrict_to_owner(&temporary)?;
        fs::rename(&temporary, path)
            .with_context(|| format!("failed to publish {}", path.display()))?;
        restrict_to_owner(path)
    }

    pub fn read(path: &Path) -> Result<Self> {
        let descriptor: Self = serde_json::from_slice(
            &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
        )?;
        if descriptor.protocol_version != DEVELOPER_SESSION_PROTOCOL_VERSION {
            return Err(anyhow!(
                "Fission Developer session protocol {} is incompatible with {}",
                descriptor.protocol_version,
                DEVELOPER_SESSION_PROTOCOL_VERSION
            ));
        }
        Ok(descriptor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReloadOutcome {
    Activated {
        generation: u64,
    },
    Rejected {
        candidate_generation: u64,
        diagnostic: String,
    },
}

#[derive(Clone, Debug)]
pub struct DeveloperSessionClient {
    descriptor_path: PathBuf,
    descriptor: DeveloperSessionDescriptor,
}

impl DeveloperSessionClient {
    pub fn discover(project: impl AsRef<Path>) -> Result<Self> {
        let manifest = canonical_manifest(project.as_ref())?;
        let directory = developer_session_directory();
        let mut candidates = fs::read_dir(&directory)
            .with_context(|| format!("no Fission Developer sessions in {}", directory.display()))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let modified = entry.metadata().ok()?.modified().ok()?;
                let descriptor = DeveloperSessionDescriptor::read(&entry.path()).ok()?;
                (descriptor.project_manifest == manifest
                    && descriptor.status != DeveloperSessionStatus::Stopped)
                    .then_some((modified, entry.path(), descriptor))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.0);
        let (_, descriptor_path, descriptor) = candidates.pop().ok_or_else(|| {
            anyhow!(
                "no running Fission Developer session found for {}",
                manifest.display()
            )
        })?;
        Ok(Self {
            descriptor_path,
            descriptor,
        })
    }

    pub fn from_descriptor(path: impl Into<PathBuf>) -> Result<Self> {
        let descriptor_path = path.into();
        let descriptor = DeveloperSessionDescriptor::read(&descriptor_path)?;
        Ok(Self {
            descriptor_path,
            descriptor,
        })
    }

    pub fn descriptor(&self) -> &DeveloperSessionDescriptor {
        &self.descriptor
    }

    pub fn refresh(&mut self) -> Result<&DeveloperSessionDescriptor> {
        self.descriptor = DeveloperSessionDescriptor::read(&self.descriptor_path)?;
        Ok(&self.descriptor)
    }

    pub fn live_test_client(&self) -> LiveTestClient {
        LiveTestClient::connect_authenticated(
            self.descriptor.control_port,
            self.descriptor.bearer_token.clone(),
        )
        .with_scope(SelectorQuery::semantic_identifier(
            self.descriptor.application_scope.clone(),
        ))
    }

    pub fn active_generation(&self) -> u64 {
        self.descriptor.active_generation
    }

    pub fn wait_for_reload_after(
        &mut self,
        generation: u64,
        timeout: Duration,
    ) -> Result<ReloadOutcome> {
        let started = Instant::now();
        loop {
            let descriptor = self.refresh()?;
            if descriptor.status == DeveloperSessionStatus::Ready
                && descriptor.active_generation > generation
            {
                return Ok(ReloadOutcome::Activated {
                    generation: descriptor.active_generation,
                });
            }
            if descriptor.status == DeveloperSessionStatus::Failed {
                if let Some(candidate_generation) = descriptor.candidate_generation {
                    if candidate_generation > generation {
                        return Ok(ReloadOutcome::Rejected {
                            candidate_generation,
                            diagnostic: descriptor
                                .diagnostic
                                .clone()
                                .unwrap_or_else(|| "candidate was rejected".into()),
                        });
                    }
                }
            }
            if started.elapsed() >= timeout {
                return Err(anyhow!(
                    "timed out waiting for a Fission Developer generation after {generation}"
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

pub fn developer_session_directory() -> PathBuf {
    std::env::var_os("FISSION_DEVELOPER_SESSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("fission-developer")
                .join("sessions")
        })
}

pub fn developer_session_path(session_id: &str) -> PathBuf {
    developer_session_directory().join(format!("{session_id}.json"))
}

fn canonical_manifest(project: &Path) -> Result<PathBuf> {
    let manifest = if project.file_name().is_some_and(|name| name == "Cargo.toml") {
        project.to_owned()
    } else {
        project.join("Cargo.toml")
    };
    manifest
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", manifest.display()))
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to protect {}", path.display()))
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_round_trips_and_reload_wait_observes_activation() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("session.json");
        let manifest = temporary.path().join("Cargo.toml");
        fs::write(&manifest, "[package]\nname='fixture'\nversion='0.1.0'\n").unwrap();
        let mut descriptor = DeveloperSessionDescriptor {
            protocol_version: DEVELOPER_SESSION_PROTOCOL_VERSION,
            session_id: "session".into(),
            project_manifest: manifest.canonicalize().unwrap(),
            process_id: std::process::id(),
            control_port: 48123,
            bearer_token: "capability".into(),
            application_scope: "fission.developer.application".into(),
            status: DeveloperSessionStatus::Ready,
            active_generation: 1,
            candidate_generation: None,
            diagnostic: None,
        };
        descriptor.write_atomic(&path).unwrap();
        let mut client = DeveloperSessionClient::from_descriptor(&path).unwrap();
        descriptor.active_generation = 2;
        descriptor.write_atomic(&path).unwrap();
        assert_eq!(
            client
                .wait_for_reload_after(1, Duration::from_secs(1))
                .unwrap(),
            ReloadOutcome::Activated { generation: 2 }
        );
    }
}
