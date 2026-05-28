use fission_core::{
    HapticError, HapticImpactRequest, HapticNotificationRequest, HapticPatternRequest,
    HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN, HAPTIC_SELECTION,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::{Arc, Mutex};

#[cfg(any(target_os = "macos", target_os = "ios"))]
use objc::{class, msg_send, sel, sel_impl};

#[cfg(target_os = "ios")]
#[link(name = "UIKit", kind = "framework")]
extern "C" {}

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

#[derive(Debug, Default)]
pub struct NativeHapticHost;

pub(crate) fn native_haptic_host() -> NativeHapticHost {
    NativeHapticHost
}

impl NativeHapticHost {
    #[cfg(target_os = "macos")]
    fn perform_macos_haptic(pattern: usize) -> Result<(), HapticError> {
        unsafe {
            let manager = class!(NSHapticFeedbackManager);
            let performer: *mut objc::runtime::Object = msg_send![manager, defaultPerformer];
            if performer.is_null() {
                return Err(HapticError::unsupported("macos_haptic_feedback"));
            }
            let _: () = msg_send![
                performer,
                performFeedbackPattern: pattern
                performanceTime: 0usize
            ];
        }
        Ok(())
    }

    #[cfg(target_os = "ios")]
    fn perform_ios_impact(style: isize) -> Result<(), HapticError> {
        unsafe {
            let class = class!(UIImpactFeedbackGenerator);
            let generator: *mut objc::runtime::Object = msg_send![class, alloc];
            let generator: *mut objc::runtime::Object = msg_send![generator, initWithStyle: style];
            let _: () = msg_send![generator, prepare];
            let _: () = msg_send![generator, impactOccurred];
        }
        Ok(())
    }

    #[cfg(target_os = "ios")]
    fn perform_ios_selection() -> Result<(), HapticError> {
        unsafe {
            let class = class!(UISelectionFeedbackGenerator);
            let generator: *mut objc::runtime::Object = msg_send![class, new];
            let _: () = msg_send![generator, prepare];
            let _: () = msg_send![generator, selectionChanged];
        }
        Ok(())
    }

    #[cfg(target_os = "ios")]
    fn perform_ios_notification(kind: isize) -> Result<(), HapticError> {
        unsafe {
            let class = class!(UINotificationFeedbackGenerator);
            let generator: *mut objc::runtime::Object = msg_send![class, new];
            let _: () = msg_send![generator, prepare];
            let _: () = msg_send![generator, notificationOccurred: kind];
        }
        Ok(())
    }
}

impl HapticHost for NativeHapticHost {
    fn impact(&self, request: HapticImpactRequest) -> Result<(), HapticError> {
        #[cfg(target_os = "ios")]
        {
            let style = match request.style {
                fission_core::HapticImpactStyle::Light | fission_core::HapticImpactStyle::Soft => 0,
                fission_core::HapticImpactStyle::Medium => 1,
                fission_core::HapticImpactStyle::Heavy | fission_core::HapticImpactStyle::Rigid => {
                    2
                }
            };
            return Self::perform_ios_impact(style);
        }

        #[cfg(target_os = "macos")]
        {
            let pattern = match request.style {
                fission_core::HapticImpactStyle::Light | fission_core::HapticImpactStyle::Soft => 1,
                fission_core::HapticImpactStyle::Medium => 0,
                fission_core::HapticImpactStyle::Heavy | fission_core::HapticImpactStyle::Rigid => {
                    2
                }
            };
            return Self::perform_macos_haptic(pattern);
        }

        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = request;
            Err(HapticError::unsupported("impact"))
        }
    }

    fn notification(&self, request: HapticNotificationRequest) -> Result<(), HapticError> {
        #[cfg(target_os = "ios")]
        {
            let kind = match request.kind {
                fission_core::HapticNotificationKind::Success => 0,
                fission_core::HapticNotificationKind::Warning => 1,
                fission_core::HapticNotificationKind::Error => 2,
            };
            return Self::perform_ios_notification(kind);
        }

        #[cfg(target_os = "macos")]
        {
            let _ = request;
            return Self::perform_macos_haptic(0);
        }

        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = request;
            Err(HapticError::unsupported("notification"))
        }
    }

    fn selection(&self) -> Result<(), HapticError> {
        #[cfg(target_os = "ios")]
        {
            return Self::perform_ios_selection();
        }

        #[cfg(target_os = "macos")]
        {
            return Self::perform_macos_haptic(1);
        }

        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            Err(HapticError::unsupported("selection"))
        }
    }

    fn pattern(&self, request: HapticPatternRequest) -> Result<(), HapticError> {
        #[cfg(target_os = "ios")]
        {
            let _ = request;
            return Self::perform_ios_impact(1);
        }

        #[cfg(target_os = "macos")]
        {
            let _ = request;
            return Self::perform_macos_haptic(0);
        }

        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = request;
            Err(HapticError::unsupported("pattern"))
        }
    }
}

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
