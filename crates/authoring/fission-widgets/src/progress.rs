use fission_core::op::Fill;
use fission_core::ui::{Container, GridItem, Widget};
use serde::{Deserialize, Serialize};

/// A determinate progress bar showing completion from 0% to 100%.
///
/// Renders a track with a filled bar overlay. The bar width is set via a
/// CSS Grid percentage column. Colors and height are read from `ProgressTheme`.
///
/// # Fields
///
/// * `value` - Progress fraction from 0.0 (empty) to 1.0 (full).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ProgressBar {
    pub value: f32, // 0.0 to 1.0
}

impl From<ProgressBar> for Widget {
    fn from(component: ProgressBar) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.progress;

        let height = theme.track_style.height.unwrap_or(theme.height);
        let radius = theme.track_style.radius.unwrap_or(theme.radius);
        let track = Container::new(fission_core::ui::widgets::spacer::Spacer::default())
            .height(height)
            .bg_fill(
                theme
                    .track_style
                    .background
                    .clone()
                    .unwrap_or(Fill::Solid(theme.track_color)),
            )
            .border_radius(radius)
            .into();

        let progress_pct = (this.value * 100.0).clamp(0.0, 100.0);

        let bar = Container::new(fission_core::ui::widgets::spacer::Spacer::default())
            .height(theme.fill_style.height.unwrap_or(height))
            .bg_fill(
                theme
                    .fill_style
                    .background
                    .clone()
                    .unwrap_or(Fill::Solid(theme.bar_color)),
            )
            .border_radius(theme.fill_style.radius.unwrap_or(radius))
            .into();

        let bar_grid = fission_core::ui::Grid {
            columns: vec![
                fission_ir::op::GridTrack::Percent(progress_pct),
                fission_ir::op::GridTrack::Fr(1.0),
            ],
            rows: vec![fission_ir::op::GridTrack::Points(height)],
            children: vec![GridItem {
                col_start: fission_ir::op::GridPlacement::Line(1),
                child: bar,
                ..Default::default()
            }
            .into()],
            ..Default::default()
        }
        .into();

        Container::new(fission_core::ui::ZStack {
            children: vec![track, bar_grid],
            ..Default::default()
        })
        .height(height)
        .flex_grow(1.0)
        .into()
    }
}
