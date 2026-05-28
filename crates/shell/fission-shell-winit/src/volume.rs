use fission_core::{
    VolumeAdjustDirection, VolumeAdjustRequest, VolumeError, VolumeLevel, VolumeSetRequest,
    VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL, SET_VOLUME_LEVEL,
};
use fission_shell::async_host::AsyncRegistry;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Host-side volume-control provider.
pub trait VolumeHost: Send + Sync + 'static {
    /// Reads the current level and mute state for one logical volume stream.
    fn get_level(&self, stream: VolumeStream) -> Result<VolumeLevel, VolumeError>;
    /// Sets the level and optional mute state for one logical volume stream.
    fn set_level(&self, request: VolumeSetRequest) -> Result<VolumeLevel, VolumeError>;
    /// Adjusts one logical volume stream relative to its current state.
    fn adjust_level(&self, request: VolumeAdjustRequest) -> Result<VolumeLevel, VolumeError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedVolumeHost;

pub(crate) fn native_volume_host() -> impl VolumeHost {
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        NativeVolumeHost
    } else {
        NativeVolumeHost
    }
}

impl VolumeHost for UnsupportedVolumeHost {
    fn get_level(&self, _stream: VolumeStream) -> Result<VolumeLevel, VolumeError> {
        Err(VolumeError::unsupported("get_level"))
    }

    fn set_level(&self, _request: VolumeSetRequest) -> Result<VolumeLevel, VolumeError> {
        Err(VolumeError::unsupported("set_level"))
    }

    fn adjust_level(&self, _request: VolumeAdjustRequest) -> Result<VolumeLevel, VolumeError> {
        Err(VolumeError::unsupported("adjust_level"))
    }
}

#[derive(Debug, Default)]
pub struct NativeVolumeHost;

impl NativeVolumeHost {
    fn get_system_level(&self, stream: VolumeStream) -> Result<VolumeLevel, VolumeError> {
        if cfg!(target_os = "macos") {
            let output = Command::new("osascript")
                .args(["-e", "output volume of (get volume settings)"])
                .output()
                .map_err(volume_command_error)?;
            if !output.status.success() {
                return Err(volume_status_error(output));
            }
            let level = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u8>()
                .map_err(|error| VolumeError::new("parse_error", error.to_string()))?
                .min(100);
            return Ok(VolumeLevel {
                stream,
                level,
                muted: false,
            });
        }

        if cfg!(target_os = "linux") && command_exists("pactl") {
            let output = Command::new("pactl")
                .args(["get-sink-volume", "@DEFAULT_SINK@"])
                .output()
                .map_err(volume_command_error)?;
            if !output.status.success() {
                return Err(volume_status_error(output));
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let level = parse_pactl_percent(&text)
                .ok_or_else(|| VolumeError::new("parse_error", "failed to parse pactl volume"))?;
            let muted = self.linux_muted().unwrap_or(false);
            return Ok(VolumeLevel {
                stream,
                level,
                muted,
            });
        }

        Err(VolumeError::unsupported("get_level"))
    }

    fn set_system_level(&self, request: VolumeSetRequest) -> Result<VolumeLevel, VolumeError> {
        let level = request.level.min(100);
        if cfg!(target_os = "macos") {
            Command::new("osascript")
                .args(["-e", &format!("set volume output volume {level}")])
                .status()
                .map_err(volume_command_error)?;
            if let Some(muted) = request.muted {
                Command::new("osascript")
                    .args([
                        "-e",
                        if muted {
                            "set volume output muted true"
                        } else {
                            "set volume output muted false"
                        },
                    ])
                    .status()
                    .map_err(volume_command_error)?;
            }
            return self.get_system_level(request.stream);
        }

        if cfg!(target_os = "linux") && command_exists("pactl") {
            Command::new("pactl")
                .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{level}%")])
                .status()
                .map_err(volume_command_error)?;
            if let Some(muted) = request.muted {
                Command::new("pactl")
                    .args([
                        "set-sink-mute",
                        "@DEFAULT_SINK@",
                        if muted { "1" } else { "0" },
                    ])
                    .status()
                    .map_err(volume_command_error)?;
            }
            return self.get_system_level(request.stream);
        }

        Err(VolumeError::unsupported("set_level"))
    }

    fn linux_muted(&self) -> Result<bool, VolumeError> {
        let output = Command::new("pactl")
            .args(["get-sink-mute", "@DEFAULT_SINK@"])
            .output()
            .map_err(volume_command_error)?;
        if !output.status.success() {
            return Err(volume_status_error(output));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text.contains("yes"))
    }
}

impl VolumeHost for NativeVolumeHost {
    fn get_level(&self, stream: VolumeStream) -> Result<VolumeLevel, VolumeError> {
        self.get_system_level(stream)
    }

    fn set_level(&self, request: VolumeSetRequest) -> Result<VolumeLevel, VolumeError> {
        self.set_system_level(request)
    }

    fn adjust_level(&self, request: VolumeAdjustRequest) -> Result<VolumeLevel, VolumeError> {
        let current = self.get_level(request.stream)?;
        let level = match request.direction {
            VolumeAdjustDirection::Up => current.level.saturating_add(request.step).min(100),
            VolumeAdjustDirection::Down => current.level.saturating_sub(request.step),
        };
        self.set_level(VolumeSetRequest {
            stream: request.stream,
            level,
            muted: None,
        })
    }
}

#[derive(Debug)]
pub struct MemoryVolumeHost {
    level: Arc<Mutex<VolumeLevel>>,
}

impl Default for MemoryVolumeHost {
    fn default() -> Self {
        Self {
            level: Arc::new(Mutex::new(VolumeLevel {
                stream: VolumeStream::Media,
                level: 50,
                muted: false,
            })),
        }
    }
}

impl MemoryVolumeHost {
    pub fn current(&self) -> VolumeLevel {
        self.level.lock().unwrap().clone()
    }
}

impl VolumeHost for MemoryVolumeHost {
    fn get_level(&self, stream: VolumeStream) -> Result<VolumeLevel, VolumeError> {
        let mut level = self.level.lock().unwrap().clone();
        level.stream = stream;
        Ok(level)
    }

    fn set_level(&self, request: VolumeSetRequest) -> Result<VolumeLevel, VolumeError> {
        let mut level = self.level.lock().unwrap();
        level.stream = request.stream;
        level.level = request.level.min(100);
        if let Some(muted) = request.muted {
            level.muted = muted;
        }
        Ok(level.clone())
    }

    fn adjust_level(&self, request: VolumeAdjustRequest) -> Result<VolumeLevel, VolumeError> {
        let mut level = self.level.lock().unwrap();
        level.stream = request.stream;
        level.level = match request.direction {
            VolumeAdjustDirection::Up => level.level.saturating_add(request.step).min(100),
            VolumeAdjustDirection::Down => level.level.saturating_sub(request.step),
        };
        Ok(level.clone())
    }
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join(name))
                .find(|path| path.is_file())
        })
        .is_some()
}

fn parse_pactl_percent(text: &str) -> Option<u8> {
    text.split_whitespace()
        .find_map(|part| part.strip_suffix('%'))
        .and_then(|number| number.parse::<u16>().ok())
        .map(|level| level.min(100) as u8)
}

fn volume_command_error(error: std::io::Error) -> VolumeError {
    VolumeError::new("host_error", error.to_string())
}

fn volume_status_error(output: std::process::Output) -> VolumeError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    VolumeError::new(
        "host_error",
        if stderr.is_empty() {
            format!("volume command exited with {}", output.status)
        } else {
            stderr
        },
    )
}

pub(crate) fn register_volume_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn VolumeHost>,
) {
    let get_host = host.clone();
    async_registry.register_operation_capability(GET_VOLUME_LEVEL, move |request, _| {
        let host = get_host.clone();
        async move { host.get_level(request) }
    });

    let set_host = host.clone();
    async_registry.register_operation_capability(SET_VOLUME_LEVEL, move |request, _| {
        let host = set_host.clone();
        async move { host.set_level(request) }
    });

    async_registry.register_operation_capability(ADJUST_VOLUME_LEVEL, move |request, _| {
        let host = host.clone();
        async move { host.adjust_level(request) }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedVolumeHost;
        assert!(host.get_level(VolumeStream::Media).is_err());
    }

    #[test]
    fn memory_host_sets_and_adjusts_volume() {
        let host = MemoryVolumeHost::default();
        let set = host
            .set_level(VolumeSetRequest {
                stream: VolumeStream::Media,
                level: 80,
                muted: Some(false),
            })
            .unwrap();
        assert_eq!(set.level, 80);

        let adjusted = host
            .adjust_level(VolumeAdjustRequest {
                stream: VolumeStream::Media,
                direction: VolumeAdjustDirection::Down,
                step: 15,
            })
            .unwrap();
        assert_eq!(adjusted.level, 65);
    }

    #[test]
    fn pactl_volume_parser_extracts_first_percentage() {
        let output =
            "Volume: front-left: 32768 / 50% / -18.06 dB, front-right: 32768 / 50% / -18.06 dB";
        assert_eq!(parse_pactl_percent(output), Some(50));
        assert_eq!(parse_pactl_percent("no percentage"), None);
    }

    #[test]
    fn native_volume_reports_unsupported_when_host_has_no_mixer() {
        if cfg!(target_os = "macos") || (cfg!(target_os = "linux") && command_exists("pactl")) {
            return;
        }

        let host = NativeVolumeHost;
        assert_eq!(
            host.get_level(VolumeStream::Media).unwrap_err().code,
            "unsupported"
        );
    }
}
