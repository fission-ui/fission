use crate::components::ui::{
    is_compact, muted_text, panel_card, small_button, status_pill, title_text,
};
use crate::model::{on_select_order, CapabilityState, FieldInspectorState, SelectOrder};
use fission::prelude::*;

pub struct WorkOrderRail;

impl Widget<FieldInspectorState> for WorkOrderRail {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> impl fission::IntoWidget<FieldInspectorState> {
        fission::AnyWidget::from_node({
            let compact = is_compact(view);
            let rows = view
                .state
                .orders
                .iter()
                .map(|order| {
                    let selected = order.id == view.state.selected_order_id;
                    let action =
                        with_reducer!(ctx, SelectOrder(order.id.to_string()), on_select_order);
                    let state = if selected {
                        CapabilityState::Ready
                    } else {
                        CapabilityState::Idle
                    };
                    let card = Column {
                        gap: Some(8.0),
                        children: vec![
                            Row {
                                gap: Some(8.0),
                                children: vec![
                                    Text::new(order.id)
                                        .size(14.0)
                                        .weight(900)
                                        .color(view.env.theme.tokens.colors.text_primary)
                                        .into_node(),
                                    Spacer {
                                        flex_grow: 1.0,
                                        ..Default::default()
                                    }
                                    .into_node(),
                                    status_pill(view, order.priority, state),
                                ],
                                ..Default::default()
                            }
                            .into_node(),
                            Text::new(order.title)
                                .size(if compact { 14.0 } else { 15.0 })
                                .line_height(if compact { 19.0 } else { 21.0 })
                                .weight(800)
                                .color(view.env.theme.tokens.colors.text_primary)
                                .into_node(),
                            muted_text(view, format!("{} - {}", order.site, order.due)),
                            small_button(
                                if selected { "Selected" } else { "Open" },
                                action,
                                if selected {
                                    ButtonVariant::SecondaryColor
                                } else {
                                    ButtonVariant::Ghost
                                },
                            ),
                        ],
                        ..Default::default()
                    }
                    .into_node();
                    let mut container = Container::new(card)
                        .bg(if selected {
                            view.env.theme.tokens.colors.primary.with_alpha(26)
                        } else {
                            view.env.theme.tokens.colors.background.with_alpha(140)
                        })
                        .border(
                            if selected {
                                view.env.theme.tokens.colors.primary
                            } else {
                                view.env.theme.tokens.colors.border.with_alpha(120)
                            },
                            1.0,
                        )
                        .border_radius(18.0)
                        .padding_all(if compact { 10.0 } else { 14.0 });
                    if compact {
                        container = container.width(220.0);
                    }
                    container.into_node()
                })
                .collect();

            let order_list = if compact {
                Scroll {
                    direction: FlexDirection::Row,
                    show_scrollbar: true,
                    child: Some(Box::new(
                        Row {
                            gap: Some(12.0),
                            children: rows,
                            ..Default::default()
                        }
                        .into_node(),
                    )),
                    ..Default::default()
                }
                .into_node()
            } else {
                Column {
                    gap: Some(12.0),
                    children: rows,
                    ..Default::default()
                }
                .into_node()
            };

            panel_card(
                view,
                Column {
                    gap: Some(14.0),
                    children: vec![
                        title_text(view, "Work orders", 22.0),
                        muted_text(view, "Choose a job, then run the full inspection workflow."),
                        order_list,
                    ],
                    ..Default::default()
                }
                .into_node(),
            )
        })
    }
}
