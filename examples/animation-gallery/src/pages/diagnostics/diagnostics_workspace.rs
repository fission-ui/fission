use super::diagnostics_case::DiagnosticsCase;
use super::diagnostics_source_panel::DiagnosticsSourcePanel;
use crate::state::AnimationGalleryState;
use crate::widgets::common::{ControlsPanel, GalleryCase, InspectorPanel};
use fission::prelude::*;

const PANEL_MIN_WIDTH: f32 = 280.0;

fission_component! {
    pub(super) DiagnosticsWorkspace<AnimationGalleryState> {
        diagnostics: DiagnosticsCase,
        gallery: GalleryCase,
    } |workspace, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.m,
            children: [
                ControlsPanel(workspace.gallery.motions.to_vec()),
                Grid(
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(PANEL_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: tokens.spacing.m,
                    row_gap: tokens.spacing.m,
                    children: [
                        DiagnosticsSourcePanel(workspace.diagnostics),
                        InspectorPanel(workspace.gallery),
                    ],
                ),
            ],
        )
}
