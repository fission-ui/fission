use crate::model::{InboxState, OpenInAppLink, OpenSystemLink, ToggleBrowserDemo};
use fission::core::ui::{Container, Text, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::widgets::{HStack, Modal, ModalAction, VStack, WebView};

pub struct BrowserModal;

impl From<BrowserModal> for Widget {
    fn from(_component: BrowserModal) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let viewport_width = view.viewport_size().width.max(0.0);
        let modal_width = (viewport_width - 48.0).clamp(360.0, 820.0);
        let webview_width = (modal_width - 96.0).clamp(260.0, 680.0);
        Modal {
            id: WidgetId::explicit("browser_modal"),
            title: "Browser & Links Demo".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(
                ToggleBrowserDemo(false),
                reduce_with!(
                    (|s: &mut InboxState, a: ToggleBrowserDemo, _| s.show_browser_demo = a.0)
                ),
            )),
            width: Some(modal_width),
            content: VStack {
                spacing: Some(24.0),
                children: vec![
                    // Section 1: Embedded
                    VStack {
                        spacing: Some(8.0),
                        children: vec![
                            Text::new("Mechanism 1: Embedded Widget").size(16.0).into(),
                            Text::new("A native WebView embedded directly in the layout.")
                                .size(12.0)
                                .color(tokens.colors.text_secondary)
                                .into(),
                            Container::new(WebView {
                                id: WidgetId::explicit("demo_webview"),
                                url: view.state().browser_url.clone(),
                                user_agent: None,
                                width: Some(webview_width),
                                height: Some(300.0),
                            })
                            .width(webview_width)
                            .height(300.0)
                            .border(tokens.colors.border, 1.0)
                            .into(),
                        ],
                    }
                    .into(),
                    // Section 2: System / In-App
                    VStack {
                        spacing: Some(8.0),
                        children: vec![
                            Text::new("Mechanism 2: System / In-App").size(16.0).into(),
                            HStack {
                                spacing: Some(16.0),
                                children: vec![
                                    fission::widgets::Button {
                                        variant: fission::widgets::ButtonVariant::Outline,
                                        child: Some(Text::new("Open System Browser").into()),
                                        on_press: Some(
                                            OpenSystemLink("https://google.com".into()).into(),
                                        ),
                                        ..Default::default()
                                    }
                                    .into(),
                                    fission::widgets::Button {
                                        variant: fission::widgets::ButtonVariant::Filled,
                                        child: Some(
                                            Text::new("Open In-App (Custom Tab)")
                                                .color(tokens.colors.on_primary)
                                                .into(),
                                        ),
                                        on_press: Some(
                                            OpenInAppLink("https://fission.rs".into()).into(),
                                        ),
                                        ..Default::default()
                                    }
                                    .into(),
                                ],
                            }
                            .into(),
                        ],
                    }
                    .into(),
                ],
            }
            .into(),
            actions: vec![ModalAction {
                label: "Close".into(),
                is_primary: true,
                on_press: Some(ctx.bind(
                    ToggleBrowserDemo(false),
                    reduce_with!(
                        (|s: &mut InboxState, a: ToggleBrowserDemo, _| s.show_browser_demo = a.0)
                    ),
                )),
            }],
        }
        .into()
    }
}
