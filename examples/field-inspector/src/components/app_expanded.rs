use crate::components::main_column::InspectorMainColumn;
use crate::components::work_orders::WorkOrderRail;
use crate::model::FieldInspectorState;
use fission::prelude::*;
const WORK_ORDER_RAIL_MIN_WIDTH: f32 = 280.0;
const WORK_ORDER_RAIL_MAX_WIDTH: f32 = 330.0;
fission_component! {
    #[derive(Clone)] pub FieldInspectorExpanded<FieldInspectorState> {}
    |_app, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Container(
            SafeArea(
                id: WidgetId::explicit("field-inspector.expanded.safe-area"),
                child: Scroll(
                    id: WidgetId::explicit("field-inspector.expanded.scroll"),
                    direction: FlexDirection::Column,
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    child: Row(
                        id: WidgetId::explicit("field-inspector.expanded.content"),
                        gap: tokens.spacing.l,
                        align_items: ir_op::AlignItems::Stretch,
                        children: [
                            Container(WorkOrderRail(false)).width_length(Length::clamp(
                                Length::points(WORK_ORDER_RAIL_MIN_WIDTH), Length::percent(30.0), Length::points(WORK_ORDER_RAIL_MAX_WIDTH),
                            )),
                            Container(InspectorMainColumn(false)).flex_grow(1.0),
                        ],
                    ),
                ),
            ),
        )
        .height_length(Length::vh(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0), end: (1.0, 1.0),
            stops: vec![(0.0, tokens.colors.background), (0.55, tokens.colors.surface), (1.0, tokens.colors.background)],
            extend: Default::default(),
        })
}
