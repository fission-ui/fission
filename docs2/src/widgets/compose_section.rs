use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

const EXAMPLE: &str = r#"fission_component! {
    pub Counter<()> {
        state { count: i32 = 0 }
    }
    view(counter, _state, _cx) =>
        Button(
            child: Text(format!("Count: {}", counter.count)),
            on_press: reduce { counter.count += 1; },
        )
}"#;

fission_component! {
    pub ComposeSection<DocsState> {}
    view(_section, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-compose".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            copy("home.compose.title").size(tokens.typography.heading1_size).weight(tokens.typography.font_weight_bold),
            Row(gap: tokens.spacing.xl, align_items: ir_op::AlignItems::Start, children: [
                Code(text: EXAMPLE.to_string()),
                Column(gap: tokens.spacing.m, children: [
                    copy("home.compose.subtitle").size(tokens.typography.heading2_size).weight(tokens.typography.font_weight_bold),
                    copy("home.compose.body").color(tokens.colors.text_secondary),
                    Link::to(cx.view.env().tr("home.compose.link"), navigation::COMPOSE),
                ]),
            ]),
        ]),
    )
}
