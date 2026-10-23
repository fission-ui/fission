use crate::state::{select_source, AnimationGalleryState, SelectSource, SourceTab};
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub SourceTabs<AnimationGalleryState> {}
    |_tabs, state, cx| {
        let spacing = cx.view.env().theme.tokens.spacing.s;
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: spacing,
            children: [
                ChoiceButton("Ergonomic API".to_string(), state.source_tab == SourceTab::Ergonomic, cx.bind(SelectSource(SourceTab::Ergonomic), reduce_with!(select_source))),
                ChoiceButton("Verbose Native API".to_string(), state.source_tab == SourceTab::Native, cx.bind(SelectSource(SourceTab::Native), reduce_with!(select_source))),
            ],
        )
}
