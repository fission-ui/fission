use crate::model::{InboxState, OpenInAppLink, OpenSystemLink, StartAuth, ToggleBrowserDemo};
use fission_core::ui::{Container, Node, Text};
use fission_core::{BuildCtx, Handler, View, Widget, WidgetNodeId};
use fission_widgets::{HStack, Modal, ModalAction, VStack, WebView};

pub struct BrowserModal;

impl Widget<InboxState> for BrowserModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let viewport_width = view.viewport_size().width.max(0.0);
        let modal_width = (viewport_width - 48.0).clamp(360.0, 820.0);
        let webview_width = (modal_width - 96.0).clamp(260.0, 680.0);
        Modal {
            id: WidgetNodeId::explicit("browser_modal"),
            title: "Browser & Links Demo".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(
                ToggleBrowserDemo(false),
                (|s: &mut InboxState, a, _| s.show_browser_demo = a.0)
                    as Handler<InboxState, ToggleBrowserDemo>,
            )),
            width: Some(modal_width),
            content: Box::new(
                VStack {
                    spacing: Some(24.0),
                    children: vec![
                        // Section 1: Embedded
                        VStack {
                            spacing: Some(8.0),
                            children: vec![
                                Text::new("Mechanism 1: Embedded Widget")
                                    .size(16.0)
                                    .into_node(),
                                Text::new("A native WebView embedded directly in the layout.")
                                    .size(12.0)
                                    .color(tokens.colors.text_secondary)
                                    .into_node(),
                                Container::new(
                                    WebView {
                                        id: WidgetNodeId::explicit("demo_webview"),
                                        url: view.state.browser_url.clone(),
                                        user_agent: None,
                                    }
                                    .build(ctx, view),
                                )
                                .width(webview_width)
                                .height(300.0)
                                .border(tokens.colors.border, 1.0)
                                .into_node(),
                            ],
                        }
                        .into_node(),
                        // Section 2: System / In-App
                        VStack {
                            spacing: Some(8.0),
                            children: vec![
                                Text::new("Mechanism 2: System / In-App")
                                    .size(16.0)
                                    .into_node(),
                                HStack {
                                    spacing: Some(16.0),
                                    children: vec![
                                        fission_widgets::Button {
                                            variant: fission_widgets::ButtonVariant::Outline,
                                            child: Some(Box::new(
                                                Text::new("Open System Browser").into_node(),
                                            )),
                                            on_press: Some(
                                                OpenSystemLink("https://google.com".into()).into(),
                                            ),
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                        fission_widgets::Button {
                                            variant: fission_widgets::ButtonVariant::Filled,
                                            child: Some(Box::new(
                                                Text::new("Open In-App (Custom Tab)")
                                                    .color(tokens.colors.on_primary)
                                                    .into_node(),
                                            )),
                                            on_press: Some(
                                                OpenInAppLink("https://fission.rs".into()).into(),
                                            ),
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                    ],
                                }
                                .into_node(),
                            ],
                        }
                        .into_node(),
                        // Section 3: Auth
                        VStack {
                            spacing: Some(8.0),
                            children: vec![
                                Text::new("Mechanism 3: Secure Auth").size(16.0).into_node(),
                                fission_widgets::Button {
                                    variant: fission_widgets::ButtonVariant::Filled,
                                    child: Some(Box::new(
                                        Text::new("Log in with Provider")
                                            .color(tokens.colors.on_primary)
                                            .into_node(),
                                    )),
                                    on_press: Some(StartAuth.into()),
                                    ..Default::default()
                                }
                                .build(ctx, view),
                            ],
                        }
                        .into_node(),
                    ],
                }
                .into_node(),
            ),
            actions: vec![ModalAction {
                label: "Close".into(),
                is_primary: true,
                on_press: Some(ctx.bind(
                    ToggleBrowserDemo(false),
                    (|s: &mut InboxState, a, _| s.show_browser_demo = a.0)
                        as Handler<InboxState, ToggleBrowserDemo>,
                )),
            }],
        }
        .build(ctx, view)
    }
}
