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
        let heading_identifier = format!(
            "site-heading-3:{}",
            card.title.replace('.', "-")
        );
    } =>
    SemanticsRegion(
        identifier: Some("hub-task-card".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Center, children: [
                Icon::svg(card.icon).size(tokens.spacing.xl).color(tokens.colors.primary),
                SemanticsRegion(
                    Text(cx.view.env().tr(card.title)).weight(tokens.typography.font_weight_bold),
                    identifier: Some(heading_identifier),
                ),
            ]),
            Column(gap: tokens.spacing.s, children: [
                for (label, href) in &card.items {
                    SemanticsRegion(
                        Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                            copy(label).color(tokens.colors.primary),
                            Spacer(flex_grow: 1.0),
                            Icon::svg(material::navigation::arrow_forward::regular())
                                .size(tokens.spacing.s)
                                .color(tokens.colors.primary),
                        ]),
                        hyperlink: Some(Hyperlink::new(*href)),
                        key: *label,
                    )
                },
            ]),
        ]),
    )
}
