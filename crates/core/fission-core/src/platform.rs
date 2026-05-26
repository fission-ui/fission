//! Platform services shared by Fission shells.
//!
//! Notifications are modelled as typed host capabilities. Deep links and
//! notification responses are inbound lifecycle actions dispatched by shells.

use crate::action::{Action, ActionId};
use crate::capability::{CapabilityType, OperationCapability};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// Stable identifier for a local or scheduled notification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationId(pub String);

impl NotificationId {
    /// Creates a stable notification id.
    ///
    /// `id` should be stable for the logical notification so future calls can
    /// replace or cancel the same notification. Prefer product identifiers such
    /// as `sync-complete` or `message-42` over random values when the app needs
    /// deterministic replacement behavior.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Permission state reported by the host notification system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationPermission {
    Granted,
    Denied,
    Provisional,
    #[default]
    NotDetermined,
    Unsupported,
}

/// Request sent when an app asks the host to request notification permission.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationPermissionRequest {
    pub alerts: bool,
    pub badge: bool,
    pub sound: bool,
    pub provisional: bool,
}

/// Current notification settings known to the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub permission: NotificationPermission,
    pub alerts: bool,
    pub badge: bool,
    pub sound: bool,
    pub scheduling: bool,
    pub push: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            permission: NotificationPermission::NotDetermined,
            alerts: false,
            badge: false,
            sound: false,
            scheduling: false,
            push: false,
        }
    }
}

/// Portable notification error payload returned by hosts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationError {
    pub code: String,
    pub message: String,
}

impl NotificationError {
    /// Creates a portable notification error payload.
    ///
    /// `code` is the stable reason reducers and tests can match. `message` is a
    /// human-readable explanation for logs, diagnostics, or a developer-facing
    /// error surface.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Creates the standard unsupported notification error.
    ///
    /// `operation` should name the attempted notification operation, such as
    /// `show`, `schedule`, `register_push`, or `set_badge_count`.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "notification operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

/// Notification sound policy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSound {
    #[default]
    Default,
    Silent,
    Named(String),
}

/// Portable scheduling request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSchedule {
    #[default]
    Immediate,
    AtUnixMillis(u64),
    AfterMillis(u64),
}

/// Action button exposed by a notification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationActionButton {
    pub id: String,
    pub title: String,
    pub destructive: bool,
    pub foreground: bool,
    pub text_input: bool,
}

/// Request to show or schedule a notification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub id: NotificationId,
    pub title: String,
    pub body: String,
    pub subtitle: Option<String>,
    pub badge: Option<u32>,
    pub sound: NotificationSound,
    pub deep_link: Option<String>,
    pub actions: Vec<NotificationActionButton>,
    pub schedule: NotificationSchedule,
}

/// Success payload for show/schedule notification operations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationReceipt {
    pub id: NotificationId,
    pub scheduled: bool,
    pub delivered: bool,
}

/// Request to cancel one notification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelNotificationRequest {
    pub id: NotificationId,
}

/// Request to set or clear the app badge.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetBadgeCountRequest {
    pub count: Option<u32>,
}

/// Request to register for remote/push notifications.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushRegistrationRequest {
    pub app_server_key: Option<String>,
    pub sender_id: Option<String>,
    pub topics: Vec<String>,
}

/// Push provider used by a host registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushPlatform {
    Apple,
    Android,
    Web,
    Windows,
    Other(String),
}

/// Push registration returned by the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushRegistration {
    pub platform: PushPlatform,
    pub token: String,
    pub endpoint: Option<String>,
    pub p256dh_key: Option<String>,
    pub auth_secret: Option<String>,
}

pub struct RequestNotificationPermissionCapability;
impl OperationCapability for RequestNotificationPermissionCapability {
    type Request = NotificationPermissionRequest;
    type Ok = NotificationSettings;
    type Err = NotificationError;
}

pub struct GetNotificationSettingsCapability;
impl OperationCapability for GetNotificationSettingsCapability {
    type Request = ();
    type Ok = NotificationSettings;
    type Err = NotificationError;
}

pub struct ShowNotificationCapability;
impl OperationCapability for ShowNotificationCapability {
    type Request = NotificationRequest;
    type Ok = NotificationReceipt;
    type Err = NotificationError;
}

pub struct ScheduleNotificationCapability;
impl OperationCapability for ScheduleNotificationCapability {
    type Request = NotificationRequest;
    type Ok = NotificationReceipt;
    type Err = NotificationError;
}

pub struct CancelNotificationCapability;
impl OperationCapability for CancelNotificationCapability {
    type Request = CancelNotificationRequest;
    type Ok = ();
    type Err = NotificationError;
}

pub struct CancelAllNotificationsCapability;
impl OperationCapability for CancelAllNotificationsCapability {
    type Request = ();
    type Ok = ();
    type Err = NotificationError;
}

pub struct SetBadgeCountCapability;
impl OperationCapability for SetBadgeCountCapability {
    type Request = SetBadgeCountRequest;
    type Ok = ();
    type Err = NotificationError;
}

pub struct RegisterPushNotificationsCapability;
impl OperationCapability for RegisterPushNotificationsCapability {
    type Request = PushRegistrationRequest;
    type Ok = PushRegistration;
    type Err = NotificationError;
}

pub struct UnregisterPushNotificationsCapability;
impl OperationCapability for UnregisterPushNotificationsCapability {
    type Request = ();
    type Ok = ();
    type Err = NotificationError;
}

pub const REQUEST_NOTIFICATION_PERMISSION: CapabilityType<RequestNotificationPermissionCapability> =
    CapabilityType::new("fission.notifications.request_permission");
pub const GET_NOTIFICATION_SETTINGS: CapabilityType<GetNotificationSettingsCapability> =
    CapabilityType::new("fission.notifications.get_settings");
pub const SHOW_NOTIFICATION: CapabilityType<ShowNotificationCapability> =
    CapabilityType::new("fission.notifications.show");
pub const SCHEDULE_NOTIFICATION: CapabilityType<ScheduleNotificationCapability> =
    CapabilityType::new("fission.notifications.schedule");
pub const CANCEL_NOTIFICATION: CapabilityType<CancelNotificationCapability> =
    CapabilityType::new("fission.notifications.cancel");
pub const CANCEL_ALL_NOTIFICATIONS: CapabilityType<CancelAllNotificationsCapability> =
    CapabilityType::new("fission.notifications.cancel_all");
pub const SET_BADGE_COUNT: CapabilityType<SetBadgeCountCapability> =
    CapabilityType::new("fission.notifications.set_badge_count");
pub const REGISTER_PUSH_NOTIFICATIONS: CapabilityType<RegisterPushNotificationsCapability> =
    CapabilityType::new("fission.notifications.register_push");
pub const UNREGISTER_PUSH_NOTIFICATIONS: CapabilityType<UnregisterPushNotificationsCapability> =
    CapabilityType::new("fission.notifications.unregister_push");

/// Source that delivered an inbound deep link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeepLinkSource {
    CustomScheme,
    UniversalLink,
    AppLink,
    WebUrl,
    Notification,
    External,
    Unknown,
}

/// Inbound URL delivered by the host shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepLink {
    pub url: String,
    pub cold_start: bool,
    pub source: DeepLinkSource,
}

impl DeepLink {
    /// Creates an inbound deep link with an unknown source.
    ///
    /// `url` is stored exactly as delivered by the host. Use `source` and
    /// `cold_start` to add shell context when the link source and startup state
    /// are known.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            cold_start: false,
            source: DeepLinkSource::Unknown,
        }
    }

    /// Records whether this link launched the app from a stopped state.
    ///
    /// Use `true` when the link was delivered during app startup. Reducers can
    /// use this to decide whether to replace the initial route or treat the link
    /// as an in-session navigation request.
    pub fn cold_start(mut self, cold_start: bool) -> Self {
        self.cold_start = cold_start;
        self
    }

    /// Records how the host classified the inbound link.
    ///
    /// The source helps reducers and analytics distinguish custom schemes,
    /// universal links, app links, web URLs, notification taps, and external
    /// handoff paths without reparsing platform-specific launch data.
    pub fn source(mut self, source: DeepLinkSource) -> Self {
        self.source = source;
        self
    }
}

/// Declarative runtime filter for inbound deep links.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepLinkConfig {
    pub schemes: Vec<String>,
    pub domains: Vec<String>,
    pub path_prefixes: Vec<String>,
}

impl DeepLinkConfig {
    /// Creates an empty deep-link configuration.
    ///
    /// Add schemes, domains, and optional path prefixes before installing it in a
    /// shell. An empty config intentionally matches no URLs.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allows a custom URL scheme.
    ///
    /// `scheme` is normalized by trimming whitespace, removing a trailing colon,
    /// and lowercasing. Use this for routes such as `myapp://item/123`.
    pub fn scheme(mut self, scheme: impl Into<String>) -> Self {
        self.schemes.push(normalize_scheme(scheme.into()));
        self
    }

    /// Allows an HTTP or HTTPS domain.
    ///
    /// `domain` is normalized by trimming whitespace, removing a trailing dot,
    /// and lowercasing. Use this for app links, universal links, and web routes
    /// that should enter the Fission app.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domains.push(normalize_domain(domain.into()));
        self
    }

    /// Restricts accepted links to a path prefix.
    ///
    /// Prefixes are normalized to start with `/`. Add one or more prefixes when
    /// only part of a domain should route into the app, such as `/invite` or
    /// `/checkout`.
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefixes
            .push(normalize_path_prefix(prefix.into()));
        self
    }

    /// Returns `true` when no scheme, domain, or path rule has been configured.
    ///
    /// Empty configs match no URLs. This prevents a shell from accidentally
    /// accepting every external URL before the app has declared its routes.
    pub fn is_empty(&self) -> bool {
        self.schemes.is_empty() && self.domains.is_empty() && self.path_prefixes.is_empty()
    }

    /// Returns whether a URL is accepted by this deep-link configuration.
    ///
    /// `url` is parsed as a simple absolute URL. The URL matches when its scheme
    /// or domain is allowed and, if path prefixes were configured, its path starts
    /// with one of those prefixes.
    pub fn matches(&self, url: &str) -> bool {
        if self.is_empty() {
            return false;
        }
        let Some(parts) = ParsedUrl::parse(url) else {
            return false;
        };
        let scheme_matches = self
            .schemes
            .iter()
            .any(|scheme| scheme == &normalize_scheme(parts.scheme));
        let domain_matches = parts.host.as_deref().is_some_and(|host| {
            let host = normalize_domain(host.to_string());
            self.domains.iter().any(|domain| domain == &host)
        });
        let path_matches = self.path_prefixes.is_empty()
            || self
                .path_prefixes
                .iter()
                .any(|prefix| parts.path.starts_with(prefix));

        (scheme_matches || domain_matches) && path_matches
    }

    /// Classifies a URL according to this configuration.
    ///
    /// Use this in shell code when creating `DeepLinkReceived` actions. The
    /// result distinguishes configured custom schemes and associated domains from
    /// ordinary web URLs or external URLs.
    pub fn source_for(&self, url: &str) -> DeepLinkSource {
        let Some(parts) = ParsedUrl::parse(url) else {
            return DeepLinkSource::Unknown;
        };
        if parts.scheme == "http" || parts.scheme == "https" {
            if parts.host.as_deref().is_some_and(|host| {
                let host = normalize_domain(host.to_string());
                self.domains.iter().any(|domain| domain == &host)
            }) {
                return DeepLinkSource::UniversalLink;
            }
            return DeepLinkSource::WebUrl;
        }
        if self
            .schemes
            .iter()
            .any(|scheme| scheme == &normalize_scheme(parts.scheme))
        {
            DeepLinkSource::CustomScheme
        } else {
            DeepLinkSource::External
        }
    }
}

#[derive(Debug)]
struct ParsedUrl<'a> {
    scheme: &'a str,
    host: Option<&'a str>,
    path: String,
}

impl<'a> ParsedUrl<'a> {
    fn parse(url: &'a str) -> Option<Self> {
        let (scheme, rest) = url.split_once(':')?;
        if scheme.is_empty() {
            return None;
        }
        let mut host = None;
        let mut path = String::from("/");
        if let Some(authority_and_path) = rest.strip_prefix("//") {
            let authority_end = authority_and_path
                .find(['/', '?', '#'])
                .unwrap_or(authority_and_path.len());
            let authority = &authority_and_path[..authority_end];
            if !authority.is_empty() {
                let host_without_userinfo = authority.rsplit('@').next().unwrap_or(authority);
                let host_without_port = host_without_userinfo
                    .split_once(':')
                    .map(|(h, _)| h)
                    .unwrap_or(host_without_userinfo);
                if !host_without_port.is_empty() {
                    host = Some(host_without_port);
                }
            }
            let remainder = &authority_and_path[authority_end..];
            if remainder.starts_with('/') {
                path = remainder
                    .split(['?', '#'])
                    .next()
                    .unwrap_or("/")
                    .to_string();
            }
        } else if rest.starts_with('/') {
            path = rest.split(['?', '#']).next().unwrap_or("/").to_string();
        }
        Some(Self { scheme, host, path })
    }
}

fn normalize_scheme(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .trim()
        .trim_end_matches(':')
        .to_ascii_lowercase()
}

fn normalize_domain(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn normalize_path_prefix(value: impl AsRef<str>) -> String {
    let value = value.as_ref().trim();
    if value.is_empty() || value == "/" {
        "/".to_string()
    } else if value.starts_with('/') {
        value.to_string()
    } else {
        format!("/{value}")
    }
}

/// Built-in action dispatched by shells when a deep link is received.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepLinkReceived {
    pub link: DeepLink,
}

impl Action for DeepLinkReceived {
    fn static_id() -> ActionId {
        lazy_static! {
            static ref ID: ActionId = ActionId::from_name("fission_core::DeepLinkReceived");
        }
        *ID
    }
}

/// Response from the OS/browser after a user interacted with a notification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationResponse {
    pub notification_id: NotificationId,
    pub action_id: Option<String>,
    pub deep_link: Option<String>,
    pub user_text: Option<String>,
}

/// Built-in action dispatched by shells when a notification response arrives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationResponseReceived {
    pub response: NotificationResponse,
}

impl Action for NotificationResponseReceived {
    fn static_id() -> ActionId {
        lazy_static! {
            static ref ID: ActionId =
                ActionId::from_name("fission_core::NotificationResponseReceived");
        }
        *ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_request_round_trips() {
        let request = NotificationRequest {
            id: NotificationId::new("sync"),
            title: "Sync complete".into(),
            body: "All files are up to date".into(),
            subtitle: Some("Workspace".into()),
            badge: Some(2),
            sound: NotificationSound::Named("done".into()),
            deep_link: Some("fission://sync/results".into()),
            actions: vec![NotificationActionButton {
                id: "open".into(),
                title: "Open".into(),
                foreground: true,
                ..Default::default()
            }],
            schedule: NotificationSchedule::AfterMillis(500),
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: NotificationRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn deep_link_config_matches_schemes_domains_and_paths() {
        let config = DeepLinkConfig::new()
            .scheme("Fission")
            .domain("Example.COM")
            .path_prefix("/tasks");

        assert!(config.matches("fission://open/tasks/42"));
        assert!(config.matches("https://example.com/tasks/42?from=email"));
        assert!(!config.matches("https://example.com/projects/42"));
        assert!(!config.matches("other://open/tasks/42"));
    }

    #[test]
    fn built_in_actions_round_trip() {
        let link = DeepLinkReceived {
            link: DeepLink::new("fission://task/1")
                .cold_start(true)
                .source(DeepLinkSource::CustomScheme),
        };
        let envelope: crate::ActionEnvelope = link.clone().into();
        assert_eq!(envelope.id, DeepLinkReceived::static_id());
        assert_eq!(
            serde_json::from_slice::<DeepLinkReceived>(&envelope.payload).unwrap(),
            link
        );

        let response = NotificationResponseReceived {
            response: NotificationResponse {
                notification_id: NotificationId::new("task"),
                action_id: Some("open".into()),
                deep_link: Some("fission://task/1".into()),
                user_text: None,
            },
        };
        let envelope: crate::ActionEnvelope = response.clone().into();
        assert_eq!(envelope.id, NotificationResponseReceived::static_id());
        assert_eq!(
            serde_json::from_slice::<NotificationResponseReceived>(&envelope.payload).unwrap(),
            response
        );
    }
}
