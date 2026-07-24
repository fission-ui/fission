use crate::components::ui::{MutedText, TitleScale, TitleText};
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct SectionHeader {
    pub title: &'static str,
    pub body: &'static str,
}

impl From<SectionHeader> for Widget {
    fn from(header: SectionHeader) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();

        Column {
            gap: Some(view.env().theme.tokens.spacing.xs),
            children: widgets![
                TitleText::new(header.title, TitleScale::Section),
                MutedText::new(header.body),
            ],
            ..Default::default()
        }
        .into()
    }
}
