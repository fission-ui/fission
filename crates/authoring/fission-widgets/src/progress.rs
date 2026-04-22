use fission_core::ui::{Container, GridItem, Node};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ProgressBar {
    pub value: f32, // 0.0 to 1.0
}

impl<S: fission_core::AppState> Widget<S> for ProgressBar {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.progress;

        let track =
            Container::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
                .height(theme.height)
                .bg(theme.track_color)
                .border_radius(theme.height / 2.0)
                .into_node();

        let progress_pct = (self.value * 100.0).clamp(0.0, 100.0);

        let bar = Container::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
            .height(theme.height)
            .bg(theme.bar_color)
            .border_radius(theme.height / 2.0)
            .into_node();

        let bar_grid = fission_core::ui::Grid {
            columns: vec![
                fission_ir::op::GridTrack::Percent(progress_pct),
                fission_ir::op::GridTrack::Fr(1.0),
            ],
            rows: vec![fission_ir::op::GridTrack::Points(theme.height)],
            children: vec![GridItem {
                col_start: fission_ir::op::GridPlacement::Line(1),
                child: Box::new(bar),
                ..Default::default()
            }
            .into_node()],
            ..Default::default()
        }
        .into_node();

        Container::new(
            fission_core::ui::ZStack {
                children: vec![track, bar_grid],
                ..Default::default()
            }
            .into_node(),
        )
        .height(theme.height)
        .flex_grow(1.0)
        .into_node()
    }
}
