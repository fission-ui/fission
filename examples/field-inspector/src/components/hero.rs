use crate::components::ui::{
    ActionButton, MutedText, PanelCard, StatusPill, TitleScale, TitleText,
};
use crate::model::{
    on_select_panel, on_start_inspection, CapabilityState, FieldInspectorState, InspectorPanel,
    SelectPanel, StartInspection,
};
use fission::prelude::*;
fission_component! {
    #[derive(Clone, Copy)] pub InspectorHero<FieldInspectorState> { compact: bool }
    |hero, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        PanelCard(
            if hero.compact {
                Column(
                    gap: tokens.spacing.m,
                    children: [HeroSummary(true), HeroActions(true)],
                )
            } else {
                Row(
                    gap: tokens.spacing.l,
                    align_items: ir_op::AlignItems::Start,
                    children: [HeroSummary(false), HeroActions(false)],
                )
            }
        )
}

fission_component! {
    HeroSummary<FieldInspectorState> { compact: bool }
    |summary, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let order = state.selected_order();
    } =>
        Column(
            gap: tokens.spacing.s,
            flex_grow: 1.0_f32,
            children: [
                Row(
                    gap: tokens.spacing.s,
                    wrap: ir_op::FlexWrap::Wrap,
                    children: [
                        StatusPill("Field Inspector".to_string(), CapabilityState::Ready),
                        StatusPill(state.provider_mode.label().to_string(), state.provider_mode.state()),
                    ],
                ),
                TitleText(
                    "Capability-driven field service".to_string(),
                    if summary.compact { TitleScale::Page } else { TitleScale::Display },
                ),
                MutedText(format!("{} - {} - assigned to {}", order.id, order.site, order.assigned_to)),
                MutedText(state.provider_mode.detail().to_string()),
            ],
        )
}

fission_component! {
    HeroActions<FieldInspectorState> { compact: bool }
    |actions, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let (complete, total) = state.checklist_progress();
    } =>
        Column(
            gap: tokens.spacing.s,
            align_items: if actions.compact { ir_op::AlignItems::Start } else { ir_op::AlignItems::End },
            children: [
                StatusPill(
                    format!("Checklist {complete}/{total}"),
                    if complete == total { CapabilityState::Complete } else { CapabilityState::Pending },
                ),
                if actions.compact {
                    ActionButton(
                        "field-inspector.action.start".to_string(),
                        if state.started { "Refresh checks" } else { "Start inspection" }.to_string(),
                        with_reducer!(cx.build, StartInspection, on_start_inspection),
                        ButtonVariant::Primary,
                    )
                },
                ActionButton(
                    "field-inspector.action.review".to_string(),
                    "Review report".to_string(),
                    with_reducer!(cx.build, SelectPanel(InspectorPanel::Review), on_select_panel),
                    ButtonVariant::SecondaryColor,
                ),
            ],
        )
}
