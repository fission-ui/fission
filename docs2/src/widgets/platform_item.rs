use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub PlatformItem<DocsState> {
        props { icon: &'static str, label: &'static str, detail: &'static str }
    }
    view(item, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Center, children: [
        Icon::svg(item.icon).size(tokens.spacing.xl),
        Column(children: [
            copy(item.label).weight(tokens.typography.font_weight_semibold),
            copy(item.detail).color(tokens.colors.text_secondary),
        ]),
    ])
}
