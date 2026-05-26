#![cfg(all(
    feature = "desktop",
    not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
))]

use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct PlatformApiState {
    last_link: Option<String>,
    last_notification: Option<String>,
}

impl AppState for PlatformApiState {}

fn on_deep_link(state: &mut PlatformApiState, action: DeepLinkReceived) {
    state.last_link = Some(action.link.url);
}

fn on_notification_response(state: &mut PlatformApiState, action: NotificationResponseReceived) {
    state.last_notification = Some(action.response.notification_id.0);
}

struct PlatformApiApp;

impl Widget<PlatformApiState> for PlatformApiApp {
    fn build(&self, _ctx: &mut BuildCtx<PlatformApiState>, _view: &View<PlatformApiState>) -> Node {
        Text::new("platform api").into_node()
    }
}

#[test]
fn facade_exports_notifications_and_deep_links() {
    let _app = DesktopApp::new(PlatformApiApp)
        .with_notification_host(MemoryNotificationHost)
        .with_deep_link_config(
            DeepLinkConfig::new()
                .scheme("fission")
                .domain("example.com"),
        )
        .with_startup_deep_link(DeepLink::new("fission://open/1").cold_start(true))
        .with_startup_notification_response(NotificationResponse {
            notification_id: NotificationId::new("n1"),
            action_id: Some("open".into()),
            deep_link: Some("fission://open/1".into()),
            user_text: None,
        })
        .on_deep_link(on_deep_link as fn(&mut PlatformApiState, DeepLinkReceived))
        .on_notification_response(
            on_notification_response as fn(&mut PlatformApiState, NotificationResponseReceived),
        );
}
