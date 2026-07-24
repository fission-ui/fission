use super::{CompositionDialog, GalleryCase};
use crate::state::AnimationGalleryState;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub struct WidgetPage<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub case: GalleryCase,
    pub preview: Widget,
}

impl From<WidgetPage<'_>> for Widget {
    fn from(page: WidgetPage<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();

        Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                ui::PageHeader {
                    title: page.case.title,
                    subtitle: page.case.description,
                },
                super::ControlsPanel {
                    ctx: page.ctx,
                    state: page.state,
                    motions: page.case.motions,
                },
                super::widget_workspace::WidgetWorkspace {
                    ctx: page.ctx,
                    state: page.state,
                    case: &page.case,
                    preview: page.preview,
                },
                CompositionDialog {
                    ctx: page.ctx,
                    state: page.state,
                    case: &page.case,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
