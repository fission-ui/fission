use fission_core::{
    CancelNotificationRequest, NotificationError, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationSchedule,
    NotificationSettings, PushPlatform, PushRegistration, PushRegistrationRequest,
    SetBadgeCountRequest, CANCEL_ALL_NOTIFICATIONS, CANCEL_NOTIFICATION, GET_NOTIFICATION_SETTINGS,
    REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION, SCHEDULE_NOTIFICATION,
    SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
};
use fission_shell::async_host::AsyncRegistry;
#[cfg(target_os = "ios")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "ios")]
use std::os::raw::c_void;
#[cfg(not(target_os = "ios"))]
use std::process::Command;
use std::sync::Arc;

#[cfg(target_os = "ios")]
#[link(name = "UIKit", kind = "framework")]
extern "C" {}

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

#[derive(Debug, Default)]
pub struct NativeNotificationHost;

pub(crate) fn native_notification_host() -> impl NotificationHost {
    NativeNotificationHost
}

impl NativeNotificationHost {
    fn supported() -> bool {
        cfg!(target_os = "ios")
            || cfg!(target_os = "macos")
            || (cfg!(target_os = "linux") && command_exists("notify-send"))
    }

    fn native_settings() -> NotificationSettings {
        if Self::supported() {
            NotificationSettings {
                permission: NotificationPermission::Granted,
                alerts: true,
                badge: cfg!(any(target_os = "ios", target_os = "macos")),
                sound: true,
                scheduling: cfg!(target_os = "ios"),
                push: false,
            }
        } else {
            NotificationSettings {
                permission: NotificationPermission::Unsupported,
                ..Default::default()
            }
        }
    }

    fn show_now(&self, request: &NotificationRequest) -> Result<(), NotificationError> {
        #[cfg(target_os = "ios")]
        {
            ios_register_local_notifications();
            ios_show_local_notification(request, None);
            return Ok(());
        }

        #[cfg(not(target_os = "ios"))]
        {
            if cfg!(target_os = "macos") {
                let mut script = format!(
                    "display notification {} with title {}",
                    osascript_string(&request.body),
                    osascript_string(&request.title)
                );
                if let Some(subtitle) = request.subtitle.as_deref() {
                    script.push_str(" subtitle ");
                    script.push_str(&osascript_string(subtitle));
                }
                Command::new("osascript")
                    .arg("-e")
                    .arg(script)
                    .spawn()
                    .map_err(notification_command_error)?
                    .wait()
                    .map_err(notification_command_error)?;
                return Ok(());
            }

            if cfg!(target_os = "linux") {
                if !command_exists("notify-send") {
                    return Err(NotificationError::unsupported("show"));
                }
                Command::new("notify-send")
                    .arg(&request.title)
                    .arg(&request.body)
                    .spawn()
                    .map_err(notification_command_error)?
                    .wait()
                    .map_err(notification_command_error)?;
                return Ok(());
            }

            if cfg!(target_os = "windows") {
                return Err(NotificationError::unsupported("show_windows_toast"));
            }

            Err(NotificationError::unsupported("show"))
        }
    }
}

impl NotificationHost for NativeNotificationHost {
    fn request_permission(
        &self,
        _request: NotificationPermissionRequest,
    ) -> Result<NotificationSettings, NotificationError> {
        #[cfg(target_os = "ios")]
        ios_register_local_notifications();
        Ok(Self::native_settings())
    }

    fn settings(&self) -> Result<NotificationSettings, NotificationError> {
        Ok(Self::native_settings())
    }

    fn show(&self, request: NotificationRequest) -> Result<NotificationReceipt, NotificationError> {
        match request.schedule {
            NotificationSchedule::Immediate => {
                self.show_now(&request)?;
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: false,
                    delivered: true,
                })
            }
            _ => Err(NotificationError::unsupported("schedule")),
        }
    }

    fn schedule(
        &self,
        request: NotificationRequest,
    ) -> Result<NotificationReceipt, NotificationError> {
        match request.schedule {
            NotificationSchedule::Immediate => self.show(request),
            #[cfg(target_os = "ios")]
            NotificationSchedule::AfterMillis(ms) => {
                ios_register_local_notifications();
                ios_show_local_notification(&request, Some(ms as f64 / 1000.0));
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: true,
                    delivered: false,
                })
            }
            #[cfg(target_os = "ios")]
            NotificationSchedule::AtUnixMillis(ms) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(ms);
                ios_register_local_notifications();
                ios_show_local_notification(
                    &request,
                    Some(ms.saturating_sub(now_ms) as f64 / 1000.0),
                );
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: true,
                    delivered: false,
                })
            }
            #[cfg(not(target_os = "ios"))]
            _ => Err(NotificationError::unsupported("schedule")),
        }
    }

    fn cancel(&self, _request: CancelNotificationRequest) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("cancel"))
    }

    fn cancel_all(&self) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("cancel_all"))
    }

    fn set_badge_count(&self, request: SetBadgeCountRequest) -> Result<(), NotificationError> {
        #[cfg(target_os = "ios")]
        {
            ios_set_badge_count(request.count);
            return Ok(());
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            Err(NotificationError::unsupported("set_badge_count"))
        }
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

#[cfg(target_os = "ios")]
fn ios_register_local_notifications() {
    unsafe {
        let app: *mut objc::runtime::Object = msg_send![class!(UIApplication), sharedApplication];
        if app.is_null() {
            return;
        }
        let settings: *mut objc::runtime::Object = msg_send![
            class!(UIUserNotificationSettings),
            settingsForTypes: 7usize
            categories: std::ptr::null_mut::<objc::runtime::Object>()
        ];
        if !settings.is_null() {
            let _: () = msg_send![app, registerUserNotificationSettings: settings];
        }
    }
}

#[cfg(target_os = "ios")]
fn ios_show_local_notification(request: &NotificationRequest, delay_seconds: Option<f64>) {
    unsafe {
        let notification: *mut objc::runtime::Object = msg_send![class!(UILocalNotification), new];
        if notification.is_null() {
            return;
        }
        let title = ns_string(&request.title);
        let body = ns_string(&request.body);
        let _: () = msg_send![notification, setAlertTitle: title];
        let _: () = msg_send![notification, setAlertBody: body];
        if !matches!(request.sound, fission_core::NotificationSound::Silent) {
            let default_sound: *mut objc::runtime::Object =
                msg_send![class!(UILocalNotification), defaultSoundName];
            let _: () = msg_send![notification, setSoundName: default_sound];
        }
        if let Some(badge) = request.badge {
            let _: () = msg_send![notification, setApplicationIconBadgeNumber: badge as isize];
        }
        let app: *mut objc::runtime::Object = msg_send![class!(UIApplication), sharedApplication];
        if app.is_null() {
            return;
        }
        if let Some(delay) = delay_seconds {
            let date: *mut objc::runtime::Object =
                msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: delay.max(0.0)];
            let _: () = msg_send![notification, setFireDate: date];
            let _: () = msg_send![app, scheduleLocalNotification: notification];
        } else {
            let _: () = msg_send![app, presentLocalNotificationNow: notification];
        }
    }
}

#[cfg(target_os = "ios")]
fn ios_set_badge_count(count: Option<u32>) {
    unsafe {
        let app: *mut objc::runtime::Object = msg_send![class!(UIApplication), sharedApplication];
        if !app.is_null() {
            let _: () = msg_send![app, setApplicationIconBadgeNumber: count.unwrap_or(0) as isize];
        }
    }
}

#[cfg(target_os = "ios")]
fn ns_string(value: &str) -> *mut objc::runtime::Object {
    unsafe {
        let string: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
        msg_send![
            string,
            initWithBytes: value.as_ptr() as *const c_void
            length: value.len()
            encoding: 4usize
        ]
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

#[cfg(not(target_os = "ios"))]
fn notification_command_error(error: std::io::Error) -> NotificationError {
    NotificationError::new("host_error", error.to_string())
}

#[cfg(not(target_os = "ios"))]
fn osascript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
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

    #[test]
    fn native_host_settings_are_honest_about_support() {
        let settings = NativeNotificationHost::native_settings();
        if NativeNotificationHost::supported() {
            assert_eq!(settings.permission, NotificationPermission::Granted);
            assert!(settings.alerts);
            assert!(!settings.push);
        } else {
            assert_eq!(settings.permission, NotificationPermission::Unsupported);
            assert!(!settings.alerts);
        }
    }
}
