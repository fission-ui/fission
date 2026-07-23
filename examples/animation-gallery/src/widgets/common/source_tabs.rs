use crate::state::{select_source, AnimationGalleryState, SelectSource, SourceTab};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub struct SourceTabs<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<SourceTabs<'_>> for Widget {
    fn from(tabs: SourceTabs<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();

        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                ui::ChoiceButton {
                    ctx: tabs.ctx,
                    label: "Ergonomic API",
                    active: tabs.state.source_tab == SourceTab::Ergonomic,
                    action: SelectSource(SourceTab::Ergonomic),
                    reducer: select_source,
                },
                ui::ChoiceButton {
                    ctx: tabs.ctx,
                    label: "Verbose Native API",
                    active: tabs.state.source_tab == SourceTab::Native,
                    action: SelectSource(SourceTab::Native),
                    reducer: select_source,
                },
            ],
        }
        .into()
    }
}
