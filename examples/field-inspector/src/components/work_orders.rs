use crate::components::ui::{MutedText, PanelCard, TitleScale, TitleText};
use crate::components::work_order_card::WorkOrderCard;
use crate::model::{on_select_order, FieldInspectorState, SelectOrder};
use fission::prelude::*;
fission_component! {
    pub WorkOrderRail<FieldInspectorState> { compact: bool }
    |rail, state, cx| {
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        PanelCard(Column(
            gap: spacing.s,
            children: [
                TitleText("Work orders".to_string(), TitleScale::Section),
                MutedText("Choose a job, then run the full inspection workflow.".to_string()),
                if rail.compact {
                    Scroll(
                        direction: FlexDirection::Row,
                        show_scrollbar: true,
                        child: Row(
                            gap: spacing.m,
                            children: [
                                for order in state.orders.iter() {
                                    WorkOrderCard(
                                        order.clone(),
                                        order.id == state.selected_order_id,
                                        true,
                                        with_reducer!(cx.build, SelectOrder(order.id.to_string()), on_select_order),
                                        key: order.id,
                                    )
                                },
                            ],
                        ),
                    )
                } else {
                    Column(
                        gap: spacing.m,
                        children: [
                            for order in state.orders.iter() {
                                WorkOrderCard(
                                    order.clone(),
                                    order.id == state.selected_order_id,
                                    false,
                                    with_reducer!(cx.build, SelectOrder(order.id.to_string()), on_select_order),
                                    key: order.id,
                                )
                            },
                        ],
                    )
                },
            ],
        ))
}
