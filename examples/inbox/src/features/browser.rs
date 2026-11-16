use crate::model::navigation::set_browser_demo_open;
use crate::model::{InboxState, OpenInAppLink, OpenSystemLink, ToggleBrowserDemo};
use fission::prelude::*;

fission_component! {
    pub BrowserModal<InboxState> |_modal, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let viewport_width = cx.view.viewport_size().width.max(0.0);
        let modal_width = (viewport_width - 48.0).clamp(360.0, 820.0);
        let webview_width = (modal_width - 96.0).clamp(260.0, 680.0);
        let close = cx.bind(
            ToggleBrowserDemo(false),
            reduce_with!(set_browser_demo_open),
        );
    } =>
        Modal(
            id: WidgetId::explicit("browser_modal"),
            title: "Browser & Links Demo",
            is_open: true,
            on_dismiss: close.clone(),
            backdrop_semantics_identifier: Some("inbox.browser.backdrop".into()),
            close_semantics_identifier: Some("inbox.browser.close".into()),
            surface_semantics_identifier: Some("inbox.browser.surface".into()),
            width: modal_width,
            content: VStack(
                spacing: 24.0,
                children: [
                    VStack(
                        spacing: 8.0,
                        children: [
                            Text("Mechanism 1: Embedded Widget").size(16.0),
                            Text("A native WebView embedded directly in the layout.")
                                .size(12.0)
                                .color(tokens.colors.text_secondary),
                            Container(
                                WebView(
                                id: WidgetId::explicit("demo_webview"),
                                url: state.browser_url.clone(),
                                width: webview_width,
                                height: 300.0,
                                ),
                            )
                            .width(webview_width)
                            .height(300.0)
                            .border(tokens.colors.border, 1.0),
                        ],
                    ),
                    VStack(
                        spacing: 8.0,
                        children: [
                            Text("Mechanism 2: System / In-App").size(16.0),
                            HStack(
                                spacing: 16.0,
                                children: [
                                    Button(
                                        variant: ButtonVariant::Outline,
                                        child: Text("Open System Browser"),
                                        on_press: ActionEnvelope::from(OpenSystemLink("https://google.com".into())),
                                    ),
                                    Button(
                                        variant: ButtonVariant::Filled,
                                        child: Text("Open In-App (Custom Tab)")
                                            .color(tokens.colors.on_primary),
                                        on_press: ActionEnvelope::from(OpenInAppLink("https://fission.rs".into())),
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
            actions: vec![ModalAction {
                label: "Close".into(),
                is_primary: true,
                on_press: Some(close.clone()),
                semantics_identifier: Some("inbox.browser.done".into()),
            }],
        )
}
