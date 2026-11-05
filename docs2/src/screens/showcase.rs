use crate::state::DocsState;
use crate::widgets::SiteHeader;
use fission::prelude::*;

fission_component! {
    pub ShowcasePage<DocsState> {}
    view(_page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let label = cx.view.env().tr("showcase.frame_label");
    } =>
    Container(
        SemanticsRegion(
            identifier: Some("site-showcase-page".to_string()),
            child: Column(gap: 0.0, children: [
                SiteHeader(),
                SemanticsRegion(
                    identifier: Some("site-showcase-frame".to_string()),
                    label: Some(label.clone()),
                    child: Some(WebView {
                        id: WidgetId::explicit("docs2.showcase.web-app"),
                        url: "/showcase-app/".into(),
                        user_agent: None,
                        label: Some(label),
                        width: None,
                        height: None,
                    }
                    .into()),
                ),
            ]),
        ),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
