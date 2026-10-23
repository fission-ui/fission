use crate::components::ui::{PanelCard, SmallButton};
use crate::model::{on_select_panel, FieldInspectorState, InspectorPanel, SelectPanel};
use fission::prelude::*;
use std::sync::Arc;
const PANELS: [InspectorPanel; 6] = [
    InspectorPanel::Overview,
    InspectorPanel::Verify,
    InspectorPanel::Evidence,
    InspectorPanel::Sensors,
    InspectorPanel::Security,
    InspectorPanel::Review,
];
fission_component! {
    #[derive(Clone, Copy)] pub InspectorPanelTabs<FieldInspectorState> { compact: bool }
    |tabs, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let selected_index = PANELS.iter().position(|panel| *panel == state.panel).unwrap_or(0);
        let actions = Arc::new(PANELS.iter().map(|panel| with_reducer!(cx.build, SelectPanel(*panel), on_select_panel)).collect::<Vec<_>>());
    } =>
        PanelCard(
            if tabs.compact {
                Row(
                    gap: tokens.spacing.s,
                    wrap: ir_op::FlexWrap::Wrap,
                    children: [
                        for (index, panel) in PANELS.iter().enumerate() {
                            SmallButton(
                                panel_identifier(*panel).to_string(),
                                panel.label().to_string(),
                                actions[index].clone(),
                                if index == selected_index { ButtonVariant::Filled } else { ButtonVariant::Ghost },
                                key: panel_identifier(*panel),
                            )
                        },
                    ],
                )
            } else {
                SegmentedControl(
                    options: PANELS.iter().map(|panel| panel.label().to_string()).collect::<Vec<_>>(),
                    selected_index: selected_index,
                    on_change: Arc::new(move |index| actions[index].clone()),
                )
            }
        )
}
fn panel_identifier(panel: InspectorPanel) -> &'static str {
    match panel {
        InspectorPanel::Overview => "field-inspector.panel.overview",
        InspectorPanel::Verify => "field-inspector.panel.verify",
        InspectorPanel::Evidence => "field-inspector.panel.evidence",
        InspectorPanel::Sensors => "field-inspector.panel.sensors",
        InspectorPanel::Security => "field-inspector.panel.security",
        InspectorPanel::Review => "field-inspector.panel.review",
    }
}
