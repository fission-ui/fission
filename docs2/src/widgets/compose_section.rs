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
        let highlighted_example = format!("```rust\n{}\n```", EXAMPLE);
    } =>
    SemanticsRegion(
        identifier: Some("home-compose".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            SemanticsRegion(
                copy("home.compose.title").weight(tokens.typography.font_weight_bold),
                identifier: Some("site-heading-2:compose".to_string()),
            ),
            Row(gap: tokens.spacing.xl, align_items: ir_op::AlignItems::Start, children: [
                MarkdownContent::new(highlighted_example),
                Column(gap: tokens.spacing.m, children: [
                    SemanticsRegion(
                        copy("home.compose.subtitle").weight(tokens.typography.font_weight_bold),
                        identifier: Some("site-heading-3:compose-subtitle".to_string()),
                    ),
                    copy("home.compose.body").color(tokens.colors.text_secondary),
                    Link::to(cx.view.env().tr("home.compose.link"), navigation::COMPOSE),
                ]),
            ]),
        ]),
    )
}
