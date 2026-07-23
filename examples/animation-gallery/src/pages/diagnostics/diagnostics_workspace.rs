use super::diagnostics_case::DiagnosticsCase;
use super::diagnostics_source_panel::DiagnosticsSourcePanel;
use crate::state::AnimationGalleryState;
use crate::widgets::common::{ControlsPanel, GalleryCase, InspectorPanel};
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const PANEL_MIN_WIDTH: f32 = 280.0;

pub(super) struct DiagnosticsWorkspace<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub diagnostics: DiagnosticsCase,
    pub gallery: &'a GalleryCase,
}

impl From<DiagnosticsWorkspace<'_>> for Widget {
    fn from(workspace: DiagnosticsWorkspace<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                ControlsPanel {
                    ctx: workspace.ctx,
                    state: workspace.state,
                    motions: workspace.gallery.motions,
                },
                Grid {
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(PANEL_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(tokens.spacing.m),
                    row_gap: Some(tokens.spacing.m),
                    children: widgets![
                        DiagnosticsSourcePanel {
                            diagnostics: workspace.diagnostics,
                        },
                        InspectorPanel {
                            case: workspace.gallery,
                            state: workspace.state,
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
