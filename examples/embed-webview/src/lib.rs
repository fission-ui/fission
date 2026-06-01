use fission::prelude::*;

pub const WEBVIEW_DEMO_URL: &str = concat!(
    "data:text/html;charset=utf-8,",
    "%3C%21doctype%20html%3E%3Chtml%3E%3Chead%3E",
    "%3Cmeta%20name%3D%22viewport%22%20content%3D%22width%3Ddevice-width%2C%20initial-scale%3D1%22%3E",
    "%3Cstyle%3Ehtml%2Cbody%7Bmargin%3A0%3Bheight%3A100%25%3Bfont-family%3A-apple-system%2CBlinkMacSystemFont%2CSegoe%20UI%2Csans-serif%3Bbackground%3Alinear-gradient%28135deg%2C%23082f49%2C%2314b8a6%29%3Bcolor%3Awhite%7D.card%7Bheight%3A100%25%3Bdisplay%3Agrid%3Bplace-items%3Acenter%3Btext-align%3Acenter%7D.card%20div%7Bpadding%3A24px%3Bborder%3A1px%20solid%20rgba%28255%2C255%2C255%2C.45%29%3Bborder-radius%3A22px%3Bbackground%3Argba%28255%2C255%2C255%2C.12%29%3Bbox-shadow%3A0%2024px%2070px%20rgba%280%2C0%2C0%2C.25%29%7Dh1%7Bmargin%3A0%3Bfont-size%3A42px%3Bline-height%3A1%7Dp%7Bmargin%3A12px%200%200%3Bfont-size%3A18px%7D%3C%2Fstyle%3E",
    "%3C%2Fhead%3E%3Cbody%3E%3Cmain%20class%3D%22card%22%3E%3Cdiv%3E%3Ch1%3EWKWebView%20content%3C%2Fh1%3E",
    "%3Cp%3EDeterministic%20data%20URL%20rendered%20by%20the%20host.%3C%2Fp%3E%3C%2Fdiv%3E%3C%2Fmain%3E%3C%2Fbody%3E%3C%2Fhtml%3E",
);

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct WebViewEmbedState;

impl AppState for WebViewEmbedState {}

pub struct WebViewEmbedApp;

impl Widget<WebViewEmbedState> for WebViewEmbedApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<WebViewEmbedState>,
        view: &View<WebViewEmbedState>,
    ) -> impl fission::IntoWidget<WebViewEmbedState> {
        fission::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens.colors;
            Container::new(
                Column {
                    gap: Some(16.0),
                    children: vec![
                        Text::new("WebView embed").size(28.0).into_node(),
                        Text::new("A bounded host-backed web surface.")
                            .size(14.0)
                            .color(tokens.text_secondary)
                            .into_node(),
                        Container::new(
                            WebView {
                                id: WidgetNodeId::explicit("embed-webview.demo"),
                                url: WEBVIEW_DEMO_URL.into(),
                                user_agent: None,
                                width: Some(480.0),
                                height: Some(270.0),
                            }
                            .build_node(ctx, view),
                        )
                        .width(480.0)
                        .height(270.0)
                        .border(tokens.border, 1.0)
                        .into_node(),
                    ],
                    ..Default::default()
                }
                .into_node(),
            )
            .padding_all(32.0)
            .into_node()
        })
    }
}
