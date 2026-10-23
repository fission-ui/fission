use crate::components::ui::{MutedText, TitleScale, TitleText};
use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub SectionHeader<FieldInspectorState> { title: &'static str, body: &'static str }
    |header, _state, cx| {} =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.xs,
            children: [
                TitleText(header.title.to_string(), TitleScale::Section),
                MutedText(header.body.to_string()),
            ],
        )
}
