use super::widget_workspace::WidgetWorkspace;
use super::{CompositionDialog, ControlsPanel, GalleryCase};
use crate::state::AnimationGalleryState;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub WidgetPage<AnimationGalleryState> { case: GalleryCase, preview: Widget }
    |page, _state, cx| {
        let spacing = cx.view.env().theme.tokens.spacing.s;
    } =>
        Column(
            gap: spacing,
            children: [
                PageHeader(page.case.title.to_string(), page.case.description.to_string()),
                ControlsPanel(page.case.motions.to_vec()),
                WidgetWorkspace(page.case, page.preview),
                CompositionDialog(page.case),
            ],
        )
}
