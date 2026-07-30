#[cfg(target_os = "macos")]
use block::{Block, ConcreteBlock};
#[cfg(target_os = "macos")]
use fission_core::NotificationResponse;
use fission_core::{
    CancelNotificationRequest, NotificationError, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationSchedule,
    NotificationSettings, PushPlatform, PushRegistration, PushRegistrationRequest,
    SetBadgeCountRequest, CANCEL_ALL_NOTIFICATIONS, CANCEL_NOTIFICATION, GET_NOTIFICATION_SETTINGS,
    REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION, SCHEDULE_NOTIFICATION,
    SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
};
use fission_shell::async_host::AsyncRegistry;
#[cfg(target_os = "macos")]
use objc::declare::ClassDecl;
#[cfg(target_os = "macos")]
use objc::runtime::{Class, Object, Protocol, Sel};
#[cfg(target_os = "ios")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use std::os::raw::c_void;
#[cfg(not(target_os = "ios"))]
use std::process::Command;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::{Condvar, Mutex, OnceLock};
#[cfg(target_os = "windows")]
use windows::{
    core::{HSTRING, PCWSTR},
    Data::Xml::Dom::XmlDocument,
    Win32::{
        Foundation::{APPMODEL_ERROR_NO_PACKAGE, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS},
        Storage::Packaging::Appx::GetCurrentApplicationUserModelId,
        System::{
            Com::CoTaskMemFree,
            WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
        },
        UI::Shell::{
            GetCurrentProcessExplicitAppUserModelID, SetCurrentProcessExplicitAppUserModelID,
        },
    },
    UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier},
};

#[cfg(target_os = "ios")]
#[link(name = "UIKit", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
#[link(name = "Foundation", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
#[link(name = "UserNotifications", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
type NotificationResponseHandler = Arc<dyn Fn(NotificationResponse) + Send + Sync>;

#[cfg(target_os = "macos")]
static NOTIFICATION_RESPONSE_HANDLER: OnceLock<Mutex<Option<NotificationResponseHandler>>> =
    OnceLock::new();

/// Installs the event-loop bridge used by the native notification delegate.
#[cfg(target_os = "macos")]
pub(crate) fn install_notification_response_handler(handler: NotificationResponseHandler) {
    let slot = NOTIFICATION_RESPONSE_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut current) = slot.lock() {
        *current = Some(handler);
    }
    install_macos_notification_delegate();
}

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
pub struct NativeNotificationHost {
    #[cfg(target_os = "windows")]
    windows_app_user_model_id: Option<String>,
}

pub(crate) fn native_notification_host() -> impl NotificationHost {
    NativeNotificationHost::default()
}

#[cfg(target_os = "windows")]
pub(crate) fn native_notification_host_with_windows_app_user_model_id(
    app_user_model_id: impl Into<String>,
) -> impl NotificationHost {
    let app_user_model_id = app_user_model_id.into();
    prepare_windows_app_user_model_id(&app_user_model_id);
    NativeNotificationHost {
        windows_app_user_model_id: Some(app_user_model_id),
    }
}

impl NativeNotificationHost {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn supported() -> bool {
        cfg!(target_os = "ios")
            || cfg!(target_os = "macos")
            || (cfg!(target_os = "linux") && command_exists("notify-send"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn native_settings() -> NotificationSettings {
        if Self::supported() {
            NotificationSettings {
                permission: NotificationPermission::Granted,
                alerts: true,
                badge: cfg!(any(target_os = "ios", target_os = "macos")),
                sound: true,
                scheduling: cfg!(any(target_os = "ios", target_os = "macos"))
                    || (cfg!(target_os = "linux") && command_exists("notify-send")),
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
                #[cfg(target_os = "macos")]
                {
                    macos_deliver_notification(request, None)?;
                    return Ok(());
                }
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
                #[cfg(target_os = "windows")]
                {
                    windows_show_notification(request, self.windows_app_user_model_id.as_deref())?;
                    return Ok(());
                }
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
        #[cfg(target_os = "macos")]
        {
            macos_request_notification_permission()
        }
        #[cfg(target_os = "windows")]
        {
            windows_notification_settings(self.windows_app_user_model_id.as_deref())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            #[cfg(target_os = "ios")]
            ios_register_local_notifications();
            Ok(Self::native_settings())
        }
    }

    fn settings(&self) -> Result<NotificationSettings, NotificationError> {
        #[cfg(target_os = "macos")]
        {
            macos_notification_settings()
        }
        #[cfg(target_os = "windows")]
        {
            windows_notification_settings(self.windows_app_user_model_id.as_deref())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Ok(Self::native_settings())
        }
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
            NotificationSchedule::AfterMillis(ms) => {
                if cfg!(target_os = "macos") {
                    #[cfg(target_os = "macos")]
                    {
                        macos_deliver_notification(&request, Some(ms as f64 / 1000.0))?;
                        return Ok(NotificationReceipt {
                            id: request.id,
                            scheduled: true,
                            delivered: false,
                        });
                    }
                }
                if !(cfg!(target_os = "linux") && command_exists("notify-send")) {
                    return Err(NotificationError::unsupported("schedule"));
                }
                let id = request.id.clone();
                let request = request.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    let host = NativeNotificationHost::default();
                    let _ = host.show_now(&request);
                });
                Ok(NotificationReceipt {
                    id,
                    scheduled: true,
                    delivered: false,
                })
            }
            #[cfg(not(target_os = "ios"))]
            NotificationSchedule::AtUnixMillis(ms) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(ms);
                if cfg!(target_os = "macos") {
                    #[cfg(target_os = "macos")]
                    {
                        macos_deliver_notification(
                            &request,
                            Some(ms.saturating_sub(now_ms) as f64 / 1000.0),
                        )?;
                        return Ok(NotificationReceipt {
                            id: request.id,
                            scheduled: true,
                            delivered: false,
                        });
                    }
                }
                if !(cfg!(target_os = "linux") && command_exists("notify-send")) {
                    return Err(NotificationError::unsupported("schedule"));
                }
                let id = request.id.clone();
                let request = request.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(ms.saturating_sub(now_ms)));
                    let host = NativeNotificationHost::default();
                    let _ = host.show_now(&request);
                });
                Ok(NotificationReceipt {
                    id,
                    scheduled: true,
                    delivered: false,
                })
            }
        }
    }

    fn cancel(&self, request: CancelNotificationRequest) -> Result<(), NotificationError> {
        #[cfg(target_os = "macos")]
        {
            macos_cancel_notification(&request.id.0);
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            windows_cancel_notification(&request.id.0, self.windows_app_user_model_id.as_deref())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = request;
            Err(NotificationError::unsupported("cancel"))
        }
    }

    fn cancel_all(&self) -> Result<(), NotificationError> {
        #[cfg(target_os = "macos")]
        {
            macos_cancel_all_notifications();
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            windows_cancel_all_notifications(self.windows_app_user_model_id.as_deref())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(NotificationError::unsupported("cancel_all"))
        }
    }

    fn set_badge_count(&self, request: SetBadgeCountRequest) -> Result<(), NotificationError> {
        #[cfg(target_os = "ios")]
        {
            ios_set_badge_count(request.count);
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            macos_set_badge_count(request.count);
            return Ok(());
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
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

#[cfg(target_os = "windows")]
const WINDOWS_APP_USER_MODEL_ID_ENV: &str = "FISSION_WINDOWS_APP_USER_MODEL_ID";
#[cfg(target_os = "windows")]
const WINDOWS_TOAST_GROUP: &str = "fission";

#[cfg(any(test, target_os = "windows"))]
fn windows_toast_tag(id: &str) -> String {
    // Windows toast tags are limited to 16 characters. FNV-1a gives cancellation
    // a deterministic, process-independent tag without exposing or truncating
    // the caller's logical notification id.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(any(test, target_os = "windows"))]
fn windows_toast_xml(request: &NotificationRequest) -> String {
    let mut text = String::new();
    text.push_str("<text>");
    text.push_str(&escape_notification_xml(&request.title));
    text.push_str("</text>");
    if let Some(subtitle) = request.subtitle.as_deref() {
        text.push_str("<text>");
        text.push_str(&escape_notification_xml(subtitle));
        text.push_str("</text>");
    }
    text.push_str("<text>");
    text.push_str(&escape_notification_xml(&request.body));
    text.push_str("</text>");

    let audio = match &request.sound {
        fission_core::NotificationSound::Silent => r#"<audio silent="true"/>"#.to_string(),
        fission_core::NotificationSound::Named(sound) => {
            format!(r#"<audio src="{}"/>"#, escape_notification_xml(sound))
        }
        fission_core::NotificationSound::Default => String::new(),
    };

    format!(
        r#"<toast><visual><binding template="ToastGeneric">{text}</binding></visual>{audio}</toast>"#
    )
}

#[cfg(any(test, target_os = "windows"))]
fn escape_notification_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(any(test, target_os = "windows"))]
fn windows_settings_from_code(code: i32) -> NotificationSettings {
    let permission = if code == 0 {
        NotificationPermission::Granted
    } else {
        NotificationPermission::Denied
    };
    let enabled = matches!(permission, NotificationPermission::Granted);
    NotificationSettings {
        permission,
        alerts: enabled,
        badge: false,
        sound: enabled,
        scheduling: false,
        push: false,
    }
}

#[cfg(target_os = "windows")]
struct WindowsRuntimeGuard {
    uninitialize: bool,
}

#[cfg(target_os = "windows")]
impl WindowsRuntimeGuard {
    fn initialize() -> Result<Self, NotificationError> {
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(Self { uninitialize: true }),
            // RPC_E_CHANGED_MODE means this thread was already initialized as
            // an STA. WinRT remains available and must not be uninitialized by us.
            Err(error) if error.code().0 as u32 == 0x80010106 => Ok(Self {
                uninitialize: false,
            }),
            Err(error) => Err(windows_notification_error(
                "windows_runtime_unavailable",
                error,
            )),
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsRuntimeGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { RoUninitialize() };
        }
    }
}

#[cfg(target_os = "windows")]
enum WindowsToastIdentity {
    Packaged,
    AppUserModelId(HSTRING),
}

#[cfg(target_os = "windows")]
struct WindowsToastContext {
    notifier: ToastNotifier,
    identity: WindowsToastIdentity,
    _runtime: WindowsRuntimeGuard,
}

#[cfg(target_os = "windows")]
fn windows_toast_context(
    configured_app_user_model_id: Option<&str>,
) -> Result<WindowsToastContext, NotificationError> {
    let runtime = WindowsRuntimeGuard::initialize()?;

    if windows_process_has_package_identity()? {
        let notifier = ToastNotificationManager::CreateToastNotifier().map_err(|error| {
            windows_identity_error(
                "Windows could not create a toast notifier from the app package identity",
                error,
            )
        })?;
        return Ok(WindowsToastContext {
            notifier,
            identity: WindowsToastIdentity::Packaged,
            _runtime: runtime,
        });
    }

    if let Some(app_user_model_id) =
        configured_windows_app_user_model_id(configured_app_user_model_id)?
    {
        let app_user_model_id = HSTRING::from(app_user_model_id);
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_user_model_id)
            .map_err(|error| {
                windows_identity_error(
                    "Windows could not create a toast notifier for the configured AppUserModelID",
                    error,
                )
            })?;
        return Ok(WindowsToastContext {
            notifier,
            identity: WindowsToastIdentity::AppUserModelId(app_user_model_id),
            _runtime: runtime,
        });
    }

    Err(NotificationError::new(
        "windows_app_identity_missing",
        format!(
            "Windows local notifications require package identity or an explicit AppUserModelID. \
             Configure one with WinitApp::with_windows_app_user_model_id, set \
             {WINDOWS_APP_USER_MODEL_ID_ENV}, or set the current process AppUserModelID. \
             Ordinary desktop installers must also create a matching Start Menu shortcut."
        ),
    ))
}

#[cfg(target_os = "windows")]
fn windows_process_has_package_identity() -> Result<bool, NotificationError> {
    let mut length = 0u32;
    let status =
        unsafe { GetCurrentApplicationUserModelId(&mut length, windows::core::PWSTR::null()) };
    match status {
        ERROR_INSUFFICIENT_BUFFER | ERROR_SUCCESS => Ok(true),
        APPMODEL_ERROR_NO_PACKAGE => Ok(false),
        error => Err(NotificationError::new(
            "windows_app_identity_unavailable",
            format!(
                "Windows could not determine whether this process has package identity \
                 (Win32 error {})",
                error.0
            ),
        )),
    }
}

#[cfg(target_os = "windows")]
fn prepare_windows_app_user_model_id(app_user_model_id: &str) {
    // The process-level ID should be assigned before the first window is
    // created. Preserve package-owned identity when this same binary runs from
    // an MSIX package; the configured ID is only the unpackaged fallback.
    if matches!(windows_process_has_package_identity(), Ok(false)) {
        let _ = configure_windows_app_user_model_id(app_user_model_id);
    }
}

#[cfg(target_os = "windows")]
fn configured_windows_app_user_model_id(
    configured: Option<&str>,
) -> Result<Option<String>, NotificationError> {
    if let Some(value) = configured {
        return configure_windows_app_user_model_id(value).map(Some);
    }

    if let Some(value) = std::env::var_os(WINDOWS_APP_USER_MODEL_ID_ENV) {
        let value = value.into_string().map_err(|_| {
            NotificationError::new(
                "windows_app_user_model_id_invalid",
                format!("{WINDOWS_APP_USER_MODEL_ID_ENV} must contain valid Unicode"),
            )
        })?;
        return configure_windows_app_user_model_id(&value).map(Some);
    }

    let value = match unsafe { GetCurrentProcessExplicitAppUserModelID() } {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let result = unsafe { value.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(value.as_ptr().cast())) };
    result
        .filter(|value| !value.trim().is_empty())
        .map(|value| configure_windows_app_user_model_id(&value))
        .transpose()
}

#[cfg(target_os = "windows")]
fn configure_windows_app_user_model_id(value: &str) -> Result<String, NotificationError> {
    let value = validate_windows_app_user_model_id(value)?;
    let wide = value
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(PCWSTR::from_raw(wide.as_ptr())).map_err(
            |error| windows_notification_error("windows_app_user_model_id_invalid", error),
        )?;
    }
    Ok(value)
}

#[cfg(any(test, target_os = "windows"))]
fn validate_windows_app_user_model_id(value: &str) -> Result<String, NotificationError> {
    if value.is_empty()
        || value.encode_utf16().count() > 128
        || value.chars().any(char::is_whitespace)
    {
        return Err(NotificationError::new(
            "windows_app_user_model_id_invalid",
            "a Windows AppUserModelID must contain 1 to 128 characters and cannot contain spaces",
        ));
    }
    Ok(value.to_string())
}

#[cfg(target_os = "windows")]
fn windows_notification_settings(
    configured_app_user_model_id: Option<&str>,
) -> Result<NotificationSettings, NotificationError> {
    let context = match windows_toast_context(configured_app_user_model_id) {
        Ok(context) => context,
        Err(error) if error.code == "windows_app_identity_missing" => {
            return Ok(NotificationSettings {
                permission: NotificationPermission::Unsupported,
                ..Default::default()
            });
        }
        Err(error) => return Err(error),
    };
    let setting = context
        .notifier
        .Setting()
        .map_err(|error| windows_notification_error("windows_settings_unavailable", error))?;
    Ok(windows_settings_from_code(setting.0))
}

#[cfg(target_os = "windows")]
fn windows_show_notification(
    request: &NotificationRequest,
    configured_app_user_model_id: Option<&str>,
) -> Result<(), NotificationError> {
    let context = windows_toast_context(configured_app_user_model_id)?;
    let document = XmlDocument::new()
        .map_err(|error| windows_notification_error("windows_toast_content_invalid", error))?;
    document
        .LoadXml(&HSTRING::from(windows_toast_xml(request)))
        .map_err(|error| windows_notification_error("windows_toast_content_invalid", error))?;
    let toast = ToastNotification::CreateToastNotification(&document)
        .map_err(|error| windows_notification_error("windows_toast_unavailable", error))?;
    toast
        .SetTag(&HSTRING::from(windows_toast_tag(&request.id.0)))
        .map_err(|error| windows_notification_error("windows_toast_unavailable", error))?;
    toast
        .SetGroup(&HSTRING::from(WINDOWS_TOAST_GROUP))
        .map_err(|error| windows_notification_error("windows_toast_unavailable", error))?;
    context
        .notifier
        .Show(&toast)
        .map_err(|error| windows_notification_error("windows_toast_delivery_failed", error))
}

#[cfg(target_os = "windows")]
fn windows_cancel_notification(
    id: &str,
    configured_app_user_model_id: Option<&str>,
) -> Result<(), NotificationError> {
    let context = windows_toast_context(configured_app_user_model_id)?;
    let history = ToastNotificationManager::History()
        .map_err(|error| windows_notification_error("windows_toast_history_unavailable", error))?;
    let tag = HSTRING::from(windows_toast_tag(id));
    let group = HSTRING::from(WINDOWS_TOAST_GROUP);
    match context.identity {
        WindowsToastIdentity::Packaged => history.RemoveGroupedTag(&tag, &group),
        WindowsToastIdentity::AppUserModelId(app_user_model_id) => {
            history.RemoveGroupedTagWithId(&tag, &group, &app_user_model_id)
        }
    }
    .map_err(|error| windows_notification_error("windows_toast_cancel_failed", error))
}

#[cfg(target_os = "windows")]
fn windows_cancel_all_notifications(
    configured_app_user_model_id: Option<&str>,
) -> Result<(), NotificationError> {
    let context = windows_toast_context(configured_app_user_model_id)?;
    let history = ToastNotificationManager::History()
        .map_err(|error| windows_notification_error("windows_toast_history_unavailable", error))?;
    match context.identity {
        WindowsToastIdentity::Packaged => history.Clear(),
        WindowsToastIdentity::AppUserModelId(app_user_model_id) => {
            history.ClearWithId(&app_user_model_id)
        }
    }
    .map_err(|error| windows_notification_error("windows_toast_cancel_failed", error))
}

#[cfg(target_os = "windows")]
fn windows_identity_error(context: &str, error: windows::core::Error) -> NotificationError {
    NotificationError::new(
        "windows_app_identity_missing",
        format!(
            "{context}: {error}. Packaged apps must have package identity. Ordinary desktop apps \
             must set {WINDOWS_APP_USER_MODEL_ID_ENV} (or set the current process AppUserModelID) \
             and install a Start Menu shortcut whose System.AppUserModel.ID matches it."
        ),
    )
}

#[cfg(target_os = "windows")]
fn windows_notification_error(code: &str, error: windows::core::Error) -> NotificationError {
    NotificationError::new(
        code,
        format!("{error} (HRESULT {:#010x})", error.code().0 as u32),
    )
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

#[cfg(target_os = "macos")]
fn install_macos_notification_delegate() {
    static DELEGATE: OnceLock<usize> = OnceLock::new();

    let Some(center) = macos_notification_center() else {
        return;
    };
    unsafe {
        let delegate = *DELEGATE.get_or_init(|| {
            let delegate: *mut Object = msg_send![macos_notification_delegate_class(), new];
            delegate as usize
        }) as *mut Object;
        if !delegate.is_null() {
            let _: () = msg_send![center, setDelegate: delegate];
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_notification_center() -> Option<*mut Object> {
    if !macos_has_application_bundle() {
        return None;
    }

    unsafe {
        let center: *mut Object =
            msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];
        (!center.is_null()).then_some(center)
    }
}

#[cfg(target_os = "macos")]
fn macos_has_application_bundle() -> bool {
    unsafe {
        let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return false;
        }

        let identifier: *mut Object = msg_send![bundle, bundleIdentifier];
        let bundle_path: *mut Object = msg_send![bundle, bundlePath];
        let identifier = ns_string_to_string(identifier);
        let bundle_path = ns_string_to_string(bundle_path);

        macos_bundle_supports_user_notifications(bundle_path.as_deref(), identifier.as_deref())
    }
}

#[cfg(target_os = "macos")]
fn macos_bundle_supports_user_notifications(
    bundle_path: Option<&str>,
    bundle_identifier: Option<&str>,
) -> bool {
    bundle_path.is_some_and(|path| path.ends_with(".app"))
        && bundle_identifier.is_some_and(|identifier| !identifier.trim().is_empty())
}

#[cfg(target_os = "macos")]
fn macos_notification_delegate_class() -> &'static Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    let ptr = *CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("FissionNotificationCenterDelegate", superclass)
            .expect("register FissionNotificationCenterDelegate");
        if let Some(protocol) = Protocol::get("UNUserNotificationCenterDelegate") {
            decl.add_protocol(protocol);
        }
        unsafe {
            decl.add_method(
                sel!(userNotificationCenter:willPresentNotification:withCompletionHandler:),
                macos_notification_will_present
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut c_void),
            );
            decl.add_method(
                sel!(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:),
                macos_notification_did_receive_response
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut c_void),
            );
        }
        decl.register() as *const Class as usize
    });
    unsafe { &*(ptr as *const Class) }
}

#[cfg(target_os = "macos")]
extern "C" fn macos_notification_will_present(
    _this: &mut Object,
    _cmd: Sel,
    _center: *mut Object,
    _notification: *mut Object,
    completion_handler: *mut c_void,
) {
    let completion_handler = completion_handler.cast::<Block<(usize,), ()>>();
    if let Some(completion_handler) = unsafe { completion_handler.as_ref() } {
        // Badge, sound, list and banner presentation while the app is foregrounded.
        unsafe { completion_handler.call((1usize | 2usize | 8usize | 16usize,)) };
    }
}

#[cfg(target_os = "macos")]
extern "C" fn macos_notification_did_receive_response(
    _this: &mut Object,
    _cmd: Sel,
    _center: *mut Object,
    response: *mut Object,
    completion_handler: *mut c_void,
) {
    if let Some(response) = unsafe { decode_macos_notification_response(response) } {
        let handler = NOTIFICATION_RESPONSE_HANDLER
            .get()
            .and_then(|slot| slot.lock().ok())
            .and_then(|handler| handler.clone());
        if let Some(handler) = handler {
            handler(response);
        }
    }
    let completion_handler = completion_handler.cast::<Block<(), ()>>();
    if let Some(completion_handler) = unsafe { completion_handler.as_ref() } {
        unsafe { completion_handler.call(()) };
    }
}

#[cfg(target_os = "macos")]
unsafe fn decode_macos_notification_response(
    response: *mut Object,
) -> Option<NotificationResponse> {
    if response.is_null() {
        return None;
    }
    let notification: *mut Object = msg_send![response, notification];
    let request: *mut Object = msg_send![notification, request];
    if request.is_null() {
        return None;
    }
    let identifier: *mut Object = msg_send![request, identifier];
    let notification_id = ns_string_to_string(identifier)?;
    let action_identifier: *mut Object = msg_send![response, actionIdentifier];
    let action_id = ns_string_to_string(action_identifier).and_then(normalize_action_id);
    let content: *mut Object = msg_send![request, content];
    let user_info: *mut Object = msg_send![content, userInfo];
    let deep_link = if user_info.is_null() {
        None
    } else {
        let key = ns_string("fission_deep_link");
        let value: *mut Object = msg_send![user_info, objectForKey: key];
        ns_string_to_string(value)
    };
    let user_text = if msg_send![response, respondsToSelector: sel!(userText)] {
        let value: *mut Object = msg_send![response, userText];
        ns_string_to_string(value)
    } else {
        None
    };
    Some(NotificationResponse {
        notification_id: fission_core::NotificationId::new(notification_id),
        action_id,
        deep_link,
        user_text,
    })
}

#[cfg(target_os = "macos")]
fn normalize_action_id(action_id: String) -> Option<String> {
    match action_id.as_str() {
        "com.apple.UNNotificationDefaultActionIdentifier"
        | "com.apple.UNNotificationDismissActionIdentifier" => None,
        _ => Some(action_id),
    }
}

#[cfg(target_os = "macos")]
fn macos_request_notification_permission() -> Result<NotificationSettings, NotificationError> {
    let pair = Arc::new((Mutex::new(None), Condvar::new()));
    let pair_for_block = pair.clone();
    let block = ConcreteBlock::new(move |granted: bool, _error: *mut objc::runtime::Object| {
        let (lock, cvar) = &*pair_for_block;
        if let Ok(mut result) = lock.lock() {
            *result = Some(granted);
            cvar.notify_all();
        }
    })
    .copy();
    let center = macos_notification_center()
        .ok_or_else(|| NotificationError::unsupported("notifications"))?;
    unsafe {
        let options = 1usize | 2usize | 4usize;
        let _: () = msg_send![
            center,
            requestAuthorizationWithOptions: options
            completionHandler: &*block
        ];
    }
    let (lock, cvar) = &*pair;
    let guard = lock.lock().unwrap();
    let (guard, _) = cvar
        .wait_timeout_while(guard, std::time::Duration::from_secs(30), |value| {
            value.is_none()
        })
        .unwrap();
    let granted = (*guard).unwrap_or(false);
    Ok(NotificationSettings {
        permission: if granted {
            NotificationPermission::Granted
        } else {
            NotificationPermission::Denied
        },
        alerts: granted,
        badge: granted,
        sound: granted,
        scheduling: granted,
        push: false,
    })
}

#[cfg(target_os = "macos")]
fn macos_notification_settings() -> Result<NotificationSettings, NotificationError> {
    let pair = Arc::new((Mutex::new(None), Condvar::new()));
    let pair_for_block = pair.clone();
    let block = ConcreteBlock::new(move |settings: *mut objc::runtime::Object| {
        let status: i64 = if settings.is_null() {
            0
        } else {
            unsafe { msg_send![settings, authorizationStatus] }
        };
        let permission = match status {
            2 => NotificationPermission::Granted,
            3 | 4 => NotificationPermission::Provisional,
            1 => NotificationPermission::Denied,
            _ => NotificationPermission::NotDetermined,
        };
        let enabled = matches!(
            permission,
            NotificationPermission::Granted | NotificationPermission::Provisional
        );
        let (lock, cvar) = &*pair_for_block;
        if let Ok(mut result) = lock.lock() {
            *result = Some(NotificationSettings {
                permission,
                alerts: enabled,
                badge: enabled,
                sound: enabled,
                scheduling: enabled,
                push: false,
            });
            cvar.notify_all();
        }
    })
    .copy();
    let center = macos_notification_center()
        .ok_or_else(|| NotificationError::unsupported("notifications"))?;
    unsafe {
        let _: () = msg_send![center, getNotificationSettingsWithCompletionHandler: &*block];
    }
    let (lock, cvar) = &*pair;
    let guard = lock.lock().unwrap();
    let (guard, _) = cvar
        .wait_timeout_while(guard, std::time::Duration::from_secs(30), |value| {
            value.is_none()
        })
        .unwrap();
    Ok(guard.clone().unwrap_or(NotificationSettings {
        permission: NotificationPermission::NotDetermined,
        ..Default::default()
    }))
}

#[cfg(target_os = "macos")]
fn macos_deliver_notification(
    request: &NotificationRequest,
    delay_seconds: Option<f64>,
) -> Result<(), NotificationError> {
    let settings = macos_notification_settings()?;
    match settings.permission {
        NotificationPermission::Granted | NotificationPermission::Provisional => {}
        NotificationPermission::NotDetermined => {
            return Err(NotificationError::new(
                "permission_not_determined",
                "request macOS notification permission before showing a notification",
            ));
        }
        NotificationPermission::Denied => {
            return Err(NotificationError::new(
                "permission_denied",
                "macOS notification permission is not granted",
            ));
        }
        NotificationPermission::Unsupported => {
            return Err(NotificationError::unsupported("show"));
        }
    }

    let pair = Arc::new((Mutex::new(None), Condvar::new()));
    let pair_for_block = pair.clone();
    let block = ConcreteBlock::new(move |error: *mut objc::runtime::Object| {
        let message = if error.is_null() {
            None
        } else {
            Some(macos_error_description(error))
        };
        let (lock, cvar) = &*pair_for_block;
        if let Ok(mut result) = lock.lock() {
            *result = Some(message);
            cvar.notify_all();
        }
    })
    .copy();

    unsafe {
        let content: *mut objc::runtime::Object =
            msg_send![class!(UNMutableNotificationContent), new];
        if content.is_null() {
            return Err(NotificationError::unsupported("notification_content"));
        }
        let title = ns_string(&request.title);
        let body = ns_string(&request.body);
        let _: () = msg_send![content, setTitle: title];
        let _: () = msg_send![content, setBody: body];
        if let Some(subtitle) = request.subtitle.as_deref() {
            let subtitle = ns_string(subtitle);
            let _: () = msg_send![content, setSubtitle: subtitle];
        }
        if !matches!(request.sound, fission_core::NotificationSound::Silent) {
            let sound: *mut objc::runtime::Object =
                msg_send![class!(UNNotificationSound), defaultSound];
            let _: () = msg_send![content, setSound: sound];
        }
        if let Some(badge) = request.badge {
            let badge: *mut objc::runtime::Object =
                msg_send![class!(NSNumber), numberWithUnsignedInteger: badge as usize];
            let _: () = msg_send![content, setBadge: badge];
        }
        if let Some(deep_link) = request.deep_link.as_deref() {
            let key = ns_string("fission_deep_link");
            let value = ns_string(deep_link);
            let user_info: *mut objc::runtime::Object =
                msg_send![class!(NSDictionary), dictionaryWithObject: value forKey: key];
            let _: () = msg_send![content, setUserInfo: user_info];
        }

        let trigger: *mut objc::runtime::Object = if let Some(delay) = delay_seconds {
            msg_send![
                class!(UNTimeIntervalNotificationTrigger),
                triggerWithTimeInterval: delay.max(1.0)
                repeats: false
            ]
        } else {
            std::ptr::null_mut()
        };
        let identifier = ns_string(&request.id.0);
        let notification_request: *mut objc::runtime::Object = msg_send![
            class!(UNNotificationRequest),
            requestWithIdentifier: identifier
            content: content
            trigger: trigger
        ];
        if notification_request.is_null() {
            return Err(NotificationError::unsupported("notification_request"));
        }
        let center = macos_notification_center()
            .ok_or_else(|| NotificationError::unsupported("notifications"))?;
        let _: () = msg_send![center, addNotificationRequest: notification_request withCompletionHandler: &*block];
    }

    let (lock, cvar) = &*pair;
    let guard = lock.lock().unwrap();
    let (guard, _) = cvar
        .wait_timeout_while(guard, std::time::Duration::from_secs(30), |value| {
            value.is_none()
        })
        .unwrap();
    if let Some(Some(message)) = guard.clone() {
        Err(NotificationError::new("host_error", message))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn macos_error_description(error: *mut objc::runtime::Object) -> String {
    unsafe {
        let description: *mut objc::runtime::Object = msg_send![error, localizedDescription];
        ns_string_to_string(description).unwrap_or_else(|| "macOS notification error".into())
    }
}

#[cfg(target_os = "macos")]
fn macos_cancel_notification(id: &str) {
    let Some(center) = macos_notification_center() else {
        return;
    };
    unsafe {
        let identifier = ns_string(id);
        let ids: *mut objc::runtime::Object =
            msg_send![class!(NSArray), arrayWithObject: identifier];
        let _: () = msg_send![center, removePendingNotificationRequestsWithIdentifiers: ids];
        let _: () = msg_send![center, removeDeliveredNotificationsWithIdentifiers: ids];
    }
}

#[cfg(target_os = "macos")]
fn macos_cancel_all_notifications() {
    let Some(center) = macos_notification_center() else {
        return;
    };
    unsafe {
        let _: () = msg_send![center, removeAllPendingNotificationRequests];
        let _: () = msg_send![center, removeAllDeliveredNotifications];
    }
}

#[cfg(target_os = "macos")]
fn macos_set_badge_count(count: Option<u32>) {
    unsafe {
        let app: *mut objc::runtime::Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return;
        }
        let dock_tile: *mut objc::runtime::Object = msg_send![app, dockTile];
        if dock_tile.is_null() {
            return;
        }
        let label = count
            .filter(|count| *count > 0)
            .map(|count| ns_string(&count.to_string()))
            .unwrap_or(std::ptr::null_mut());
        let _: () = msg_send![dock_tile, setBadgeLabel: label];
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
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

#[cfg(target_os = "macos")]
fn ns_string_to_string(value: *mut objc::runtime::Object) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe {
        let ptr: *const std::os::raw::c_char = msg_send![value, UTF8String];
        (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_string_lossy().into_owned())
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

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
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

    #[test]
    fn windows_toast_tags_are_stable_and_fit_the_platform_limit() {
        assert_eq!(
            windows_toast_tag("build-finished"),
            windows_toast_tag("build-finished")
        );
        assert_ne!(
            windows_toast_tag("build-finished"),
            windows_toast_tag("deploy-finished")
        );
        assert_eq!(windows_toast_tag("build-finished").len(), 16);
    }

    #[test]
    fn windows_toast_xml_escapes_visible_content_and_sound() {
        let xml = windows_toast_xml(&NotificationRequest {
            title: "Build & test".into(),
            body: "Ready <now>".into(),
            subtitle: Some(r#"Branch "main""#.into()),
            sound: fission_core::NotificationSound::Named(
                "ms-winsoundevent:Notification.Default".into(),
            ),
            ..Default::default()
        });
        assert!(xml.contains("<text>Build &amp; test</text>"));
        assert!(xml.contains("<text>Branch &quot;main&quot;</text>"));
        assert!(xml.contains("<text>Ready &lt;now&gt;</text>"));
        assert!(
            xml.contains(r#"<audio src="ms-winsoundevent:Notification.Default"/>"#),
            "{xml}"
        );
    }

    #[test]
    fn windows_notification_settings_do_not_claim_unsupported_features() {
        let enabled = windows_settings_from_code(0);
        assert_eq!(enabled.permission, NotificationPermission::Granted);
        assert!(enabled.alerts);
        assert!(enabled.sound);
        assert!(!enabled.badge);
        assert!(!enabled.scheduling);
        assert!(!enabled.push);

        let disabled = windows_settings_from_code(2);
        assert_eq!(disabled.permission, NotificationPermission::Denied);
        assert!(!disabled.alerts);
        assert!(!disabled.sound);
    }

    #[test]
    fn windows_app_user_model_id_enforces_platform_shape() {
        assert_eq!(
            validate_windows_app_user_model_id("ExampleCompany.ExampleApp").unwrap(),
            "ExampleCompany.ExampleApp"
        );
        assert_eq!(
            validate_windows_app_user_model_id("Example Company.ExampleApp")
                .unwrap_err()
                .code,
            "windows_app_user_model_id_invalid"
        );
        assert_eq!(
            validate_windows_app_user_model_id(" ExampleCompany.ExampleApp")
                .unwrap_err()
                .code,
            "windows_app_user_model_id_invalid"
        );
        assert_eq!(
            validate_windows_app_user_model_id(&"a".repeat(129))
                .unwrap_err()
                .code,
            "windows_app_user_model_id_invalid"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_default_notification_actions_are_not_exposed_as_product_actions() {
        assert_eq!(
            normalize_action_id("com.apple.UNNotificationDefaultActionIdentifier".into()),
            None
        );
        assert_eq!(
            normalize_action_id("com.apple.UNNotificationDismissActionIdentifier".into()),
            None
        );
        assert_eq!(
            normalize_action_id("approve".into()),
            Some("approve".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_notification_center_requires_an_identified_app_bundle() {
        assert!(!macos_bundle_supports_user_notifications(
            Some("/tmp/debug"),
            None,
        ));
        assert!(!macos_bundle_supports_user_notifications(
            Some("/Applications/Example.app"),
            None,
        ));
        assert!(!macos_bundle_supports_user_notifications(
            Some("/tmp/debug"),
            Some("dev.fission.example"),
        ));
        assert!(macos_bundle_supports_user_notifications(
            Some("/Applications/Example.app"),
            Some("dev.fission.example"),
        ));
    }
}
