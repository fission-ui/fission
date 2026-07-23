use crate::components::main_column::InspectorMainColumn;
use crate::components::work_orders::WorkOrderRail;
use crate::model::FieldInspectorState;
use fission::prelude::*;

#[derive(Clone)]
pub struct FieldInspectorCompact;

impl From<FieldInspectorCompact> for Widget {
    fn from(_: FieldInspectorCompact) -> Self {
        let (_ctx, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;

        Container::new(SafeArea {
            id: Some(WidgetId::explicit("field-inspector.compact.safe-area")),
            child: Scroll {
                id: Some(WidgetId::explicit("field-inspector.compact.scroll")),
                direction: FlexDirection::Column,
                show_scrollbar: true,
                flex_grow: 1.0,
                child: Some(
                    Column {
                        id: Some(WidgetId::explicit("field-inspector.compact.content")),
                        gap: Some(tokens.spacing.m),
                        children: widgets![
                            InspectorMainColumn { compact: true },
                            WorkOrderRail { compact: true },
                        ],
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
        })
        .height_length(Length::vh(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 1.0),
            stops: vec![
                (0.0, tokens.colors.background),
                (0.55, tokens.colors.surface),
                (1.0, tokens.colors.background),
            ],
        })
        .into()
    }
}
