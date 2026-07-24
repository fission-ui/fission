use super::composer_controls::ComposerControls;
use super::composition_data::{composition_expression, composition_source};
use super::composition_preview::CompositionPreview;
use super::composition_summary_grid::CompositionSummaryGrid;
use super::expression_panel::ExpressionPanel;
use super::playback_controls::PlaybackControls;
use crate::state::AnimationGalleryState;
use crate::style::{BORDER, SURFACE};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub(super) struct CompositionWorkbench<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub path: &'a str,
}

impl From<CompositionWorkbench<'_>> for Widget {
    fn from(workbench: CompositionWorkbench<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let expression = composition_expression(workbench.state);
        let source = composition_source(workbench.state, workbench.path);

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ComposerControls {
                    ctx: workbench.ctx,
                    state: workbench.state,
                },
                ExpressionPanel {
                    expression: &expression,
                },
                CompositionSummaryGrid {
                    state: workbench.state,
                },
                ui::PageNote {
                    title: "Composition behavior",
                    body: "Add atoms in any order and repeat atoms as often as needed. Different tracks are retained. If two atoms write the same stable slot/property/phase, the later atom wins.",
                },
                CompositionPreview {
                    ctx: workbench.ctx,
                    state: workbench.state,
                },
                ui::CodeBlock { source: &source },
                PlaybackControls {
                    ctx: workbench.ctx,
                    state: workbench.state,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
