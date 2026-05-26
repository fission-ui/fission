use fission_core::{
    HapticError, HapticImpactRequest, HapticNotificationRequest, HapticPatternRequest,
    HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN, HAPTIC_SELECTION,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::{Arc, Mutex};

/// Host-side haptic feedback provider used by shell capability registration.
pub trait HapticHost: Send + Sync + 'static {
    /// Plays impact feedback with the requested strength.
    fn impact(&self, request: HapticImpactRequest) -> Result<(), HapticError>;
    /// Plays success, warning, or error notification feedback.
    fn notification(&self, request: HapticNotificationRequest) -> Result<(), HapticError>;
    /// Plays lightweight selection-change feedback.
    fn selection(&self) -> Result<(), HapticError>;
    /// Plays a bounded custom haptic pattern.
    fn pattern(&self, request: HapticPatternRequest) -> Result<(), HapticError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedHapticHost;

impl HapticHost for UnsupportedHapticHost {
    fn impact(&self, _request: HapticImpactRequest) -> Result<(), HapticError> {
        Err(HapticError::unsupported("impact"))
    }

    fn notification(&self, _request: HapticNotificationRequest) -> Result<(), HapticError> {
        Err(HapticError::unsupported("notification"))
    }

    fn selection(&self) -> Result<(), HapticError> {
        Err(HapticError::unsupported("selection"))
    }

    fn pattern(&self, _request: HapticPatternRequest) -> Result<(), HapticError> {
        Err(HapticError::unsupported("pattern"))
    }
}

#[derive(Debug, Default)]
pub struct MemoryHapticHost {
    calls: Arc<Mutex<Vec<String>>>,
}

impl MemoryHapticHost {
    pub fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

impl HapticHost for MemoryHapticHost {
    fn impact(&self, _request: HapticImpactRequest) -> Result<(), HapticError> {
        self.calls.lock().unwrap().push("impact".into());
        Ok(())
    }

    fn notification(&self, _request: HapticNotificationRequest) -> Result<(), HapticError> {
        self.calls.lock().unwrap().push("notification".into());
        Ok(())
    }

    fn selection(&self) -> Result<(), HapticError> {
        self.calls.lock().unwrap().push("selection".into());
        Ok(())
    }

    fn pattern(&self, _request: HapticPatternRequest) -> Result<(), HapticError> {
        self.calls.lock().unwrap().push("pattern".into());
        Ok(())
    }
}

pub(crate) fn register_haptic_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn HapticHost>,
) {
    let impact_host = host.clone();
    async_registry.register_operation_capability(HAPTIC_IMPACT, move |request, _| {
        let host = impact_host.clone();
        async move { host.impact(request) }
    });

    let notification_host = host.clone();
    async_registry.register_operation_capability(HAPTIC_NOTIFICATION, move |request, _| {
        let host = notification_host.clone();
        async move { host.notification(request) }
    });

    let selection_host = host.clone();
    async_registry.register_operation_capability(HAPTIC_SELECTION, move |(), _| {
        let host = selection_host.clone();
        async move { host.selection() }
    });

    async_registry.register_operation_capability(HAPTIC_PATTERN, move |request, _| {
        let host = host.clone();
        async move { host.pattern(request) }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::HapticImpactStyle;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedHapticHost;
        assert!(host.selection().is_err());
    }

    #[test]
    fn memory_host_records_calls() {
        let host = MemoryHapticHost::default();
        host.impact(HapticImpactRequest {
            style: HapticImpactStyle::Heavy,
        })
        .unwrap();
        host.selection().unwrap();
        assert_eq!(host.calls(), vec!["impact", "selection"]);
    }
}
