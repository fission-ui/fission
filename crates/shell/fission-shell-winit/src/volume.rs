use fission_core::{
    VolumeAdjustDirection, VolumeAdjustRequest, VolumeError, VolumeLevel, VolumeSetRequest,
    VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL, SET_VOLUME_LEVEL,
};
use fission_shell::async_host::AsyncRegistry;
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
}
