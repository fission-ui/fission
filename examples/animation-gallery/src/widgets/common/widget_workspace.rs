use super::{GalleryCase, InspectorPanel, SourceTabs};
use crate::state::{AnimationGalleryState, SourceTab};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

const PANEL_MIN_WIDTH: f32 = 280.0;

fission_component! {
    pub(super) WidgetWorkspace<AnimationGalleryState> { case: GalleryCase, preview: Widget }
    |workspace, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let source = match state.source_tab {
            SourceTab::Ergonomic => workspace.case.ergonomic_source,
            SourceTab::Native => workspace.case.native_source,
        };
    } =>
        Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(PANEL_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: tokens.spacing.s,
            row_gap: tokens.spacing.s,
            children: [
                Container(
                    Column(
                        gap: tokens.spacing.s,
                        children: [
                            SectionTitle("Live Preview".to_string()),
                            workspace.preview.clone(),
                            SourceTabs(),
                            CodeBlock(source.to_string()),
                        ],
                    ),
                )
                .padding_all(tokens.spacing.s)
                .border(style::border(), 1.0)
                .border_radius(tokens.radii.xl)
                .bg(style::surface()),
                InspectorPanel(workspace.case),
            ],
        )
}
