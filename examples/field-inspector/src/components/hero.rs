use crate::components::ui::{
    ActionButton, MutedText, PanelCard, StatusPill, TitleScale, TitleText,
};
use crate::model::{
    on_select_panel, on_start_inspection, CapabilityState, FieldInspectorState, InspectorPanel,
    SelectPanel, StartInspection,
};
use fission::prelude::*;

#[derive(Clone, Copy)]
pub struct InspectorHero {
    pub compact: bool,
}

impl From<InspectorHero> for Widget {
    fn from(hero: InspectorHero) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let order = view.state().selected_order();
        let start = with_reducer!(ctx, StartInspection, on_start_inspection);
        let review = with_reducer!(ctx, SelectPanel(InspectorPanel::Review), on_select_panel);
        let (complete, total) = view.state().checklist_progress();

        let summary: Widget = Column {
            gap: Some(tokens.spacing.s),
            flex_grow: 1.0,
            children: widgets![
                Row {
                    gap: Some(tokens.spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        StatusPill::new("Field Inspector", CapabilityState::Ready),
                        StatusPill::new(
                            view.state().provider_mode.label(),
                            view.state().provider_mode.state(),
                        ),
                    ],
                    ..Default::default()
                },
                TitleText::new(
                    "Capability-driven field service",
                    if hero.compact {
                        TitleScale::Page
                    } else {
                        TitleScale::Display
                    },
                ),
                MutedText::new(format!(
                    "{} - {} - assigned to {}",
                    order.id, order.site, order.assigned_to
                )),
                MutedText::new(view.state().provider_mode.detail()),
            ],
            ..Default::default()
        }
        .into();

        let mut action_children = widgets![StatusPill::new(
            format!("Checklist {complete}/{total}"),
            if complete == total {
                CapabilityState::Complete
            } else {
                CapabilityState::Pending
            },
        )];
        if hero.compact {
            action_children.push(
                ActionButton::new(
                    "field-inspector.action.start",
                    if view.state().started {
                        "Refresh checks"
                    } else {
                        "Start inspection"
                    },
                    start,
                    ButtonVariant::Primary,
                )
                .into(),
            );
        }
        action_children.push(
            ActionButton::new(
                "field-inspector.action.review",
                "Review report",
                review,
                ButtonVariant::SecondaryColor,
            )
            .into(),
        );

        let actions: Widget = Column {
            gap: Some(tokens.spacing.s),
            align_items: if hero.compact {
                ir_op::AlignItems::Start
            } else {
                ir_op::AlignItems::End
            },
            children: action_children,
            ..Default::default()
        }
        .into();

        let content: Widget = if hero.compact {
            Column {
                gap: Some(tokens.spacing.m),
                children: widgets![summary, actions],
                ..Default::default()
            }
            .into()
        } else {
            Row {
                gap: Some(tokens.spacing.l),
                align_items: ir_op::AlignItems::Start,
                children: widgets![summary, actions],
                ..Default::default()
            }
            .into()
        };

        PanelCard::new(content).into()
    }
}
