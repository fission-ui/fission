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

pub struct OverviewPanel;

impl From<OverviewPanel> for Widget {
    fn from(_panel: OverviewPanel) -> Self {
        let (_ctx, view) = fission::build::current::<FieldInspectorState>();
        let order = view.state().selected_order();
        let weather_snapshot = view.state().weather.clone();
        let spacing = &view.env().theme.tokens.spacing;

        let (complete, total) = view.state().checklist_progress();
        let hero: Widget = PanelCard::new(Column {
            gap: Some(spacing.l),
            children: widgets![
                Responsive::new(OverviewHeadingCompact)
                    .id(WidgetId::explicit("field-inspector.overview.heading"))
                    .case(ResponsiveCase::min_width(
                        OVERVIEW_EXPANDED_BREAKPOINT,
                        OverviewHeadingExpanded,
                    )),
                ResponsiveGrid::new(widgets![
                    Metric::new("Site", order.site),
                    Metric::new("Asset", order.asset.id),
                    Metric::new("Checklist", format!("{complete}/{total}")),
                ])
                .item_min_width(METRIC_MIN_WIDTH),
                SoftPanel::new(
                    Responsive::new(OverviewAssetCompact)
                        .id(WidgetId::explicit("field-inspector.overview.asset"))
                        .case(ResponsiveCase::min_width(
                            OVERVIEW_EXPANDED_BREAKPOINT,
                            OverviewAssetExpanded,
                        )),
                ),
            ],
            ..Default::default()
        })
        .into();

        Column {
            gap: Some(spacing.l),
            children: widgets![
                hero,
                WeatherCard {
                    snapshot: weather_snapshot,
                },
                SemanticsRegion::new(CapabilityOverview)
                    .identifier("field-inspector.capability-readiness")
                    .label("Capability readiness"),
            ],
            ..Default::default()
        }
        .into()
    }
}
