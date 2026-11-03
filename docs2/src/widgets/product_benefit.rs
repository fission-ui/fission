use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub ProductBenefit<DocsState> {
        props {
            label_key: &'static str,
        }
    }
    view(benefit, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some(benefit.label_key.to_string()),
        child: Row(
            gap: tokens.spacing.m,
            align_items: ir_op::AlignItems::Start,
            children: [
                Icon::svg(material::navigation::check::regular())
                    .size(tokens.spacing.l)
                    .color(tokens.colors.primary),
                copy(benefit.label_key)
                    .color(tokens.colors.text_secondary)
                    .wrap(true),
            ],
        ),
    )
}
