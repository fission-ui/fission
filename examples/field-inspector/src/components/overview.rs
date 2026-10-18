use crate::components::capability_overview::CapabilityOverview;
use crate::components::overview_asset_compact::OverviewAssetCompact;
use crate::components::overview_asset_expanded::OverviewAssetExpanded;
use crate::components::overview_heading_compact::OverviewHeadingCompact;
use crate::components::overview_heading_expanded::OverviewHeadingExpanded;
use crate::components::ui::{Metric, PanelCard, ResponsiveGrid, SoftPanel};
use crate::components::weather_card::WeatherCard;
use crate::model::FieldInspectorState;
use fission::prelude::*;
const OVERVIEW_EXPANDED_BREAKPOINT: f32 = 760.0;
const METRIC_MIN_WIDTH: f32 = 160.0;
fission_component! {
    pub OverviewPanel<FieldInspectorState> {}
    |_panel, state, cx| {
        let order = state.selected_order();
        let weather_snapshot = state.weather.clone();
        let spacing = &cx.view.env().theme.tokens.spacing;
        let (complete, total) = state.checklist_progress();
    } =>
        Column(
            gap: spacing.l,
            children: [
                PanelCard(Column(
                    gap: spacing.l,
                    children: [
                        Responsive::new(OverviewHeadingCompact::new()).id(WidgetId::explicit("field-inspector.overview.heading")).case(ResponsiveCase::min_width(OVERVIEW_EXPANDED_BREAKPOINT, OverviewHeadingExpanded::new())),
                        ResponsiveGrid(vec![
                            Metric::new("Site".to_string(), order.site.to_string()).into(),
                            Metric::new("Asset".to_string(), order.asset.id.to_string()).into(),
                            Metric::new("Checklist".to_string(), format!("{complete}/{total}")).into(),
                        ], METRIC_MIN_WIDTH),
                        SoftPanel(Responsive::new(OverviewAssetCompact::new()).id(WidgetId::explicit("field-inspector.overview.asset")).case(ResponsiveCase::min_width(OVERVIEW_EXPANDED_BREAKPOINT, OverviewAssetExpanded::new())).into()),
                    ],
                )),
                WeatherCard(weather_snapshot),
                SemanticsRegion::new(CapabilityOverview::new()).identifier("field-inspector.capability-readiness").label("Capability readiness"),
            ],
        )
}
