use crate::state::DocsState;
use crate::widgets::PlatformItem;
use fission::prelude::*;

fission_component! {
    pub PlatformStrip<DocsState> {}
    view(_strip, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-platforms".to_string()),
        label: Some(cx.view.env().tr("home.platform.label")),
        child: Row(
            children: [
                PlatformItem(material::hardware::laptop_mac::regular(), "home.platform.macos", "home.platform.native"),
                PlatformItem(material::hardware::desktop_windows::regular(), "home.platform.windows", "home.platform.native"),
                PlatformItem(material::hardware::computer::regular(), "home.platform.linux", "home.platform.native"),
                PlatformItem(material::action::language::regular(), "home.platform.web", "home.platform.wasm"),
                PlatformItem(material::hardware::smartphone::regular(), "home.platform.android", "home.platform.native"),
                PlatformItem(material::hardware::phone_iphone::regular(), "home.platform.ios", "home.platform.native"),
            ],
        ),
    )
}
