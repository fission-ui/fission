use crate::components::active_panel::ActiveInspectorPanel;
use crate::components::hero::InspectorHero;
use crate::components::panel_tabs::InspectorPanelTabs;
use crate::model::FieldInspectorState;
use fission::prelude::*;

#[derive(Clone, Copy)]
pub struct InspectorMainColumn {
    pub compact: bool,
}

impl From<InspectorMainColumn> for Widget {
    fn from(column: InspectorMainColumn) -> Self {
        let (_ctx, view) = fission::build::current::<FieldInspectorState>();
        let spacing = &view.env().theme.tokens.spacing;

        Column {
            gap: Some(spacing.l),
            children: widgets![
                InspectorHero {
                    compact: column.compact,
                },
                InspectorPanelTabs {
                    compact: column.compact,
                },
                ActiveInspectorPanel,
            ],
            ..Default::default()
        }
        .into()
    }
}
