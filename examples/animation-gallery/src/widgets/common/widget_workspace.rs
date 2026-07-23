use super::{GalleryCase, InspectorPanel, SourceTabs};
use crate::state::{AnimationGalleryState, SourceTab};
use crate::style::{BORDER, SURFACE};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const PANEL_MIN_WIDTH: f32 = 280.0;

pub(super) struct WidgetWorkspace<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub case: &'a GalleryCase,
    pub preview: Widget,
}

impl From<WidgetWorkspace<'_>> for Widget {
    fn from(workspace: WidgetWorkspace<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let source = match workspace.state.source_tab {
            SourceTab::Ergonomic => workspace.case.ergonomic_source,
            SourceTab::Native => workspace.case.native_source,
        };

        Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(PANEL_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(tokens.spacing.s),
            row_gap: Some(tokens.spacing.s),
            children: widgets![
                Container::new(Column {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        ui::SectionTitle {
                            title: "Live Preview",
                        },
                        workspace.preview,
                        SourceTabs {
                            ctx: workspace.ctx,
                            state: workspace.state,
                        },
                        ui::CodeBlock { source },
                    ],
                    ..Default::default()
                })
                .padding_all(tokens.spacing.s)
                .border(BORDER, 1.0)
                .border_radius(tokens.radii.xl)
                .bg(SURFACE),
                InspectorPanel {
                    case: workspace.case,
                    state: workspace.state,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
