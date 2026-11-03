use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub HubPathStep<DocsState> {
        props {
            number: u8,
            title: &'static str,
            detail: &'static str,
            href: &'static str,
        }
    }
    view(step, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("hub-path-step".to_string()),
        child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
            SemanticsRegion(
                label: Some(format!("{}", step.number)),
                child: Text(step.number.to_string()),
                identifier: Some(if step.number == 1 {
                    "hub-path-number-current".to_string()
                } else {
                    "hub-path-number".to_string()
                }),
            ),
            Column(gap: tokens.spacing.xs, children: [
                Link::to(cx.view.env().tr(step.title), step.href),
                copy(step.detail).color(tokens.colors.text_secondary),
            ]),
        ]),
    )
}
