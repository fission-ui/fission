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
pub struct NativeNotificationHost;

pub(crate) fn native_notification_host() -> impl NotificationHost {
    NativeNotificationHost
}

impl NativeNotificationHost {
    #[cfg(any(test, not(target_os = "macos")))]
    fn supported() -> bool {
        cfg!(target_os = "ios")
            || cfg!(target_os = "macos")
            || (cfg!(target_os = "linux") && command_exists("notify-send"))
    }

    #[cfg(any(test, not(target_os = "macos")))]
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
        #[cfg(target_os = "macos")]
        {
            macos_request_notification_permission()
        }
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(not(target_os = "macos"))]
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
                    let host = NativeNotificationHost;
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
                    let host = NativeNotificationHost;
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
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(not(target_os = "macos"))]
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
