use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub HubTaskCard<DocsState> {
        props {
            icon: &'static str,
            title: &'static str,
            items: Vec<(&'static str, &'static str)>,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("hub-task-card".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Center, children: [
                Icon::svg(card.icon).size(tokens.spacing.xl).color(tokens.colors.primary),
                Text(cx.view.env().tr(card.title)).weight(tokens.typography.font_weight_bold),
                Spacer(flex_grow: 1.0),
                Icon::svg(material::navigation::chevron_right::regular())
                    .size(tokens.spacing.m)
                    .color(tokens.colors.text_secondary),
            ]),
            Column(gap: tokens.spacing.s, children: [
                for (label, href) in &card.items {
                    SemanticsRegion(
                        Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                            Icon::svg(material::navigation::chevron_right::regular())
                                .size(tokens.spacing.s)
                                .color(tokens.colors.text_secondary),
                            copy(label).color(tokens.colors.text_secondary),
                        ]),
                        hyperlink: Some(Hyperlink::new(*href)),
                        key: *label,
                    )
                },
            ]),
        ]),
    )
}
