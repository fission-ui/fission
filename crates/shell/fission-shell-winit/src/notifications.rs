use fission_core::{
    CancelNotificationRequest, NotificationError, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationSchedule,
    NotificationSettings, PushPlatform, PushRegistration, PushRegistrationRequest,
    SetBadgeCountRequest, CANCEL_ALL_NOTIFICATIONS, CANCEL_NOTIFICATION, GET_NOTIFICATION_SETTINGS,
    REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION, SCHEDULE_NOTIFICATION,
    SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side notification provider used by the shell capability registry.
pub trait NotificationHost: Send + Sync + 'static {
    /// Requests permission for notification features such as alerts, badges, or sound.
    ///
    /// Implementations should map the typed request to the platform prompt and
    /// return the resulting settings without assuming permission was granted.
    fn request_permission(
        &self,
        request: NotificationPermissionRequest,
    ) -> Result<NotificationSettings, NotificationError>;

    /// Returns current notification settings without showing a platform prompt.
    ///
    /// Use this to report permission state, delivery support, scheduling support,
    /// badge support, and push support to reducers.
    fn settings(&self) -> Result<NotificationSettings, NotificationError>;

    /// Displays an immediate local notification.
    ///
    /// `request` contains the stable id, visible text, badge, sound, deep link,
    /// and action buttons. Return a receipt only after the host accepted the
    /// notification request.
    fn show(&self, request: NotificationRequest) -> Result<NotificationReceipt, NotificationError>;

    /// Schedules a local notification for later delivery.
    ///
    /// Implementations should persist or hand off the schedule according to the
    /// platform notification model and return an error when scheduled delivery is
    /// unavailable.
    fn schedule(
        &self,
        request: NotificationRequest,
    ) -> Result<NotificationReceipt, NotificationError>;

    /// Cancels one notification by id.
    ///
    /// `request.id` is the id originally used to show or schedule the
    /// notification. Hosts may treat an already-missing notification as success.
    fn cancel(&self, request: CancelNotificationRequest) -> Result<(), NotificationError>;

    /// Cancels all notifications owned by this app where the platform allows it.
    fn cancel_all(&self) -> Result<(), NotificationError>;

    /// Sets or clears the app badge count.
    ///
    /// `None` clears the badge. `Some(count)` asks the host to show the supplied
    /// count using the target platform badge mechanism.
    fn set_badge_count(&self, request: SetBadgeCountRequest) -> Result<(), NotificationError>;

    /// Registers this app instance for remote or push notification delivery.
    ///
    /// Provider credentials remain in host configuration. The request carries
    /// public registration inputs and the result returns token or endpoint data.
    fn register_push(
        &self,
        request: PushRegistrationRequest,
    ) -> Result<PushRegistration, NotificationError>;

    /// Removes or invalidates this app instance from remote notification delivery.
    fn unregister_push(&self) -> Result<(), NotificationError>;
}

/// Default provider used until a shell installs a platform-specific host.
#[derive(Debug, Default)]
pub struct UnsupportedNotificationHost;

impl NotificationHost for UnsupportedNotificationHost {
    fn request_permission(
        &self,
        _request: NotificationPermissionRequest,
    ) -> Result<NotificationSettings, NotificationError> {
        Ok(NotificationSettings {
            permission: NotificationPermission::Unsupported,
            ..Default::default()
        })
    }

    fn settings(&self) -> Result<NotificationSettings, NotificationError> {
        Ok(NotificationSettings {
            permission: NotificationPermission::Unsupported,
            ..Default::default()
        })
    }

    fn show(
        &self,
        _request: NotificationRequest,
    ) -> Result<NotificationReceipt, NotificationError> {
        Err(NotificationError::unsupported("show"))
    }

    fn schedule(
        &self,
        _request: NotificationRequest,
    ) -> Result<NotificationReceipt, NotificationError> {
        Err(NotificationError::unsupported("schedule"))
    }

    fn cancel(&self, _request: CancelNotificationRequest) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("cancel"))
    }

    fn cancel_all(&self) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("cancel_all"))
    }

    fn set_badge_count(&self, _request: SetBadgeCountRequest) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("set_badge_count"))
    }

    fn register_push(
        &self,
        _request: PushRegistrationRequest,
    ) -> Result<PushRegistration, NotificationError> {
        Err(NotificationError::unsupported("register_push"))
    }

    fn unregister_push(&self) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("unregister_push"))
    }
}

/// Minimal in-process host useful for smoke tests and non-OS environments.
#[derive(Debug, Default)]
pub struct MemoryNotificationHost;

impl NotificationHost for MemoryNotificationHost {
    fn request_permission(
        &self,
        request: NotificationPermissionRequest,
    ) -> Result<NotificationSettings, NotificationError> {
        Ok(NotificationSettings {
            permission: NotificationPermission::Granted,
            alerts: request.alerts,
            badge: request.badge,
            sound: request.sound,
            scheduling: true,
            push: false,
        })
    }

    fn settings(&self) -> Result<NotificationSettings, NotificationError> {
        Ok(NotificationSettings {
            permission: NotificationPermission::Granted,
            alerts: true,
            badge: true,
            sound: true,
            scheduling: true,
            push: false,
        })
    }

    fn show(&self, request: NotificationRequest) -> Result<NotificationReceipt, NotificationError> {
        Ok(NotificationReceipt {
            id: request.id,
            scheduled: false,
            delivered: true,
        })
    }

    fn schedule(
        &self,
        request: NotificationRequest,
    ) -> Result<NotificationReceipt, NotificationError> {
        Ok(NotificationReceipt {
            id: request.id,
            scheduled: !matches!(request.schedule, NotificationSchedule::Immediate),
            delivered: matches!(request.schedule, NotificationSchedule::Immediate),
        })
    }

    fn cancel(&self, _request: CancelNotificationRequest) -> Result<(), NotificationError> {
        Ok(())
    }

    fn cancel_all(&self) -> Result<(), NotificationError> {
        Ok(())
    }

    fn set_badge_count(&self, _request: SetBadgeCountRequest) -> Result<(), NotificationError> {
        Ok(())
    }

    fn register_push(
        &self,
        _request: PushRegistrationRequest,
    ) -> Result<PushRegistration, NotificationError> {
        Ok(PushRegistration {
            platform: PushPlatform::Other("memory".into()),
            token: "memory-push-token".into(),
            endpoint: None,
            p256dh_key: None,
            auth_secret: None,
        })
    }

    fn unregister_push(&self) -> Result<(), NotificationError> {
        Ok(())
    }
}

pub(crate) fn register_notification_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn NotificationHost>,
) {
    let request_host = host.clone();
    async_registry.register_operation_capability(
        REQUEST_NOTIFICATION_PERMISSION,
        move |request, _| {
            let host = request_host.clone();
            async move { host.request_permission(request) }
        },
    );

    let settings_host = host.clone();
    async_registry.register_operation_capability(GET_NOTIFICATION_SETTINGS, move |(), _| {
        let host = settings_host.clone();
        async move { host.settings() }
    });

    let show_host = host.clone();
    async_registry.register_operation_capability(SHOW_NOTIFICATION, move |request, _| {
        let host = show_host.clone();
        async move { host.show(request) }
    });

    let schedule_host = host.clone();
    async_registry.register_operation_capability(SCHEDULE_NOTIFICATION, move |request, _| {
        let host = schedule_host.clone();
        async move { host.schedule(request) }
    });

    let cancel_host = host.clone();
    async_registry.register_operation_capability(CANCEL_NOTIFICATION, move |request, _| {
        let host = cancel_host.clone();
        async move { host.cancel(request) }
    });

    let cancel_all_host = host.clone();
    async_registry.register_operation_capability(CANCEL_ALL_NOTIFICATIONS, move |(), _| {
        let host = cancel_all_host.clone();
        async move { host.cancel_all() }
    });

    let badge_host = host.clone();
    async_registry.register_operation_capability(SET_BADGE_COUNT, move |request, _| {
        let host = badge_host.clone();
        async move { host.set_badge_count(request) }
    });

    let push_host = host.clone();
    async_registry.register_operation_capability(REGISTER_PUSH_NOTIFICATIONS, move |request, _| {
        let host = push_host.clone();
        async move { host.register_push(request) }
    });

    async_registry.register_operation_capability(UNREGISTER_PUSH_NOTIFICATIONS, move |(), _| {
        let host = host.clone();
        async move { host.unregister_push() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::NotificationId;

    #[test]
    fn unsupported_host_reports_permission_without_panicking() {
        let host = UnsupportedNotificationHost;
        let settings = host
            .request_permission(NotificationPermissionRequest::default())
            .unwrap();
        assert_eq!(settings.permission, NotificationPermission::Unsupported);
        assert_eq!(
            host.show(NotificationRequest::default()).unwrap_err().code,
            "unsupported"
        );
    }

    #[test]
    fn memory_host_returns_receipts() {
        let host = MemoryNotificationHost;
        let receipt = host
            .show(NotificationRequest {
                id: NotificationId::new("n1"),
                title: "Title".into(),
                body: "Body".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(receipt.id, NotificationId::new("n1"));
        assert!(receipt.delivered);
    }
}
