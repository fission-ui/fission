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

#[derive(Clone, Copy)]
pub struct InspectorPanelTabs {
    pub compact: bool,
}

impl From<InspectorPanelTabs> for Widget {
    fn from(tabs: InspectorPanelTabs) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let selected_index = PANELS
            .iter()
            .position(|panel| *panel == view.state().panel)
            .unwrap_or(0);
        let actions = Arc::new(
            PANELS
                .iter()
                .map(|panel| with_reducer!(ctx, SelectPanel(*panel), on_select_panel))
                .collect::<Vec<_>>(),
        );

        if tabs.compact {
            let children = PANELS
                .iter()
                .enumerate()
                .map(|(index, panel)| {
                    SmallButton::new(
                        panel_identifier(*panel),
                        panel.label(),
                        actions[index].clone(),
                        if index == selected_index {
                            ButtonVariant::Filled
                        } else {
                            ButtonVariant::Ghost
                        },
                    )
                    .into()
                })
                .collect();
            return PanelCard::new(Row {
                gap: Some(tokens.spacing.s),
                wrap: ir_op::FlexWrap::Wrap,
                children,
                ..Default::default()
            })
            .into();
        }

        PanelCard::new(SegmentedControl {
            options: PANELS
                .iter()
                .map(|panel| panel.label().to_string())
                .collect(),
            selected_index,
            on_change: Some(Arc::new(move |index| actions[index].clone())),
        })
        .into()
    }
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
