use crate::components::active_panel::ActiveInspectorPanel;
use crate::components::hero::InspectorHero;
use crate::components::panel_tabs::InspectorPanelTabs;
use crate::model::FieldInspectorState;
use fission::prelude::*;
fission_component! {
    #[derive(Clone, Copy)]
    pub InspectorMainColumn<FieldInspectorState> { compact: bool }
    |column, _state, cx| {} =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.l,
            children: [InspectorHero(column.compact), InspectorPanelTabs(column.compact), ActiveInspectorPanel()],
        )
}
