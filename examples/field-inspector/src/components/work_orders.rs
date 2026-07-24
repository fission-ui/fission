use crate::components::ui::{MutedText, PanelCard, TitleScale, TitleText};
use crate::components::work_order_card::WorkOrderCard;
use crate::model::{on_select_order, FieldInspectorState, SelectOrder};
use fission::prelude::*;

pub struct WorkOrderRail {
    pub compact: bool,
}

impl From<WorkOrderRail> for Widget {
    fn from(rail: WorkOrderRail) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let spacing = &view.env().theme.tokens.spacing;
        let rows = view
            .state()
            .orders
            .iter()
            .map(|order| {
                let selected = order.id == view.state().selected_order_id;
                let action = with_reducer!(ctx, SelectOrder(order.id.to_string()), on_select_order);
                WorkOrderCard {
                    order: order.clone(),
                    selected,
                    compact: rail.compact,
                    action,
                }
                .into()
            })
            .collect();

        let order_list: Widget = if rail.compact {
            Scroll {
                direction: FlexDirection::Row,
                show_scrollbar: true,
                child: Some(
                    Row {
                        gap: Some(spacing.m),
                        children: rows,
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            }
            .into()
        } else {
            Column {
                gap: Some(spacing.m),
                children: rows,
                ..Default::default()
            }
            .into()
        };

        PanelCard::new(Column {
            gap: Some(spacing.s),
            children: widgets![
                TitleText::new("Work orders", TitleScale::Section),
                MutedText::new("Choose a job, then run the full inspection workflow."),
                order_list,
            ],
            ..Default::default()
        })
        .into()
    }
}
