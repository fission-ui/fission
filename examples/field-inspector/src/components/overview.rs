use crate::api::WEATHER_JOB;
use crate::components::status::CapabilityOverview;
use crate::components::ui::{
    action_button, body_text, is_compact, metric, muted_text, panel_card, responsive_grid,
    soft_panel, status_pill, title_text, usable_width,
};
use crate::model::{
    on_start_inspection, on_weather_failed, on_weather_loaded, CapabilityState,
    FieldInspectorState, StartInspection, WeatherFailed, WeatherLoaded,
};
use fission::core::ResourceKey;
use fission::prelude::*;

pub struct OverviewPanel;

impl Widget<FieldInspectorState> for OverviewPanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> Node {
        let order = view.state.selected_order();
        let start = with_reducer!(ctx, StartInspection, on_start_inspection);
        let weather_ok = with_reducer!(ctx, WeatherLoaded, on_weather_loaded);
        let weather_err = with_reducer!(ctx, WeatherFailed, on_weather_failed);
        let request = view.state.weather_request();
        let snapshot = view.state.weather.clone();
        let weather = FutureBuilder::new(
            ResourceKey::new("field-inspector.weather"),
            WEATHER_JOB,
            request.clone(),
            snapshot,
            |ctx, view, snapshot| weather_card(ctx, view, snapshot),
        )
        .deps(request)
        .on_ok(weather_ok)
        .on_err(weather_err)
        .build(ctx, view);

        let (complete, total) = view.state.checklist_progress();
        let heading = Column {
            gap: Some(7.0),
            flex_grow: 1.0,
            align_items: ir_op::AlignItems::Start,
            children: vec![
                status_pill(view, order.priority, CapabilityState::Warning),
                title_text(
                    view,
                    order.title,
                    if is_compact(view) { 24.0 } else { 30.0 },
                ),
                body_text(view, order.summary),
            ],
            ..Default::default()
        }
        .into_node();
        let start_button = action_button(
            if view.state.started {
                "Refresh checks"
            } else {
                "Start inspection"
            },
            start,
            ButtonVariant::Primary,
        );
        let heading_block = if is_compact(view) {
            Column {
                gap: Some(12.0),
                children: vec![heading],
                ..Default::default()
            }
            .into_node()
        } else {
            Row {
                gap: Some(12.0),
                children: vec![heading, start_button],
                align_items: ir_op::AlignItems::Start,
                ..Default::default()
            }
            .into_node()
        };

        let asset_image_width = usable_width(view, if is_compact(view) { 96.0 } else { 0.0 })
            .min(if is_compact(view) { 420.0 } else { 210.0 });
        let asset_image_height = if is_compact(view) {
            asset_image_width * 0.68
        } else {
            142.0
        };
        let asset_media = Image::network(order.asset.photo_url)
            .size(asset_image_width, asset_image_height)
            .fit(ir_op::ImageFit::Cover)
            .semantic_label(order.asset.name)
            .into_node();
        let asset_details = Column {
            gap: Some(6.0),
            flex_grow: 1.0,
            children: vec![
                Text::new(order.asset.name)
                    .size(18.0)
                    .weight(900)
                    .into_node(),
                muted_text(view, order.asset.kind),
                body_text(
                    view,
                    format!(
                        "Expected barcode {} and NFC {}",
                        order.asset.expected_barcode, order.asset.expected_nfc_uri
                    ),
                ),
            ],
            ..Default::default()
        }
        .into_node();
        let asset_block = if is_compact(view) {
            Column {
                gap: Some(12.0),
                children: vec![asset_media, asset_details],
                align_items: ir_op::AlignItems::Start,
                ..Default::default()
            }
            .into_node()
        } else {
            Row {
                gap: Some(14.0),
                children: vec![asset_media, asset_details],
                align_items: ir_op::AlignItems::Start,
                ..Default::default()
            }
            .into_node()
        };

        let hero = panel_card(
            view,
            Column {
                gap: Some(18.0),
                children: vec![
                    heading_block,
                    responsive_grid(
                        view,
                        vec![
                            metric(view, "Site", order.site),
                            metric(view, "Asset", order.asset.id),
                            metric(view, "Checklist", format!("{complete}/{total}")),
                        ],
                        3,
                    ),
                    soft_panel(view, asset_block),
                ],
                ..Default::default()
            }
            .into_node(),
        );

        Column {
            gap: Some(18.0),
            children: vec![hero, weather, CapabilityOverview.build(ctx, view)],
            ..Default::default()
        }
        .into_node()
    }
}

fn weather_card(
    ctx: &mut BuildCtx<FieldInspectorState>,
    view: &View<FieldInspectorState>,
    snapshot: &AsyncSnapshot<crate::api::WeatherSummary, crate::api::ApiError>,
) -> Node {
    let content = if let Some(weather) = snapshot.data() {
        responsive_grid(
            view,
            vec![
                metric(view, "Weather", weather.label.clone()),
                metric(
                    view,
                    "Temperature",
                    format!("{:.1} C", weather.temperature_c),
                ),
                metric(view, "Wind", format!("{:.0} kph", weather.wind_speed_kph)),
            ],
            3,
        )
    } else if snapshot.has_error() {
        body_text(view, "Live weather is unavailable; the inspection can continue with local capability providers.")
    } else {
        Row {
            gap: Some(12.0),
            children: vec![
                CircularProgress::default().build(ctx, view),
                body_text(view, "Loading live site weather from Open-Meteo..."),
            ],
            ..Default::default()
        }
        .into_node()
    };

    panel_card(
        view,
        Column {
            gap: Some(12.0),
            children: vec![
                Row {
                    gap: Some(10.0),
                    children: vec![
                        title_text(view, "Site context", 20.0),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                        status_pill(
                            view,
                            if snapshot.has_data() {
                                "Live data"
                            } else {
                                "Pending"
                            },
                            if snapshot.has_data() {
                                CapabilityState::Ready
                            } else {
                                CapabilityState::Pending
                            },
                        ),
                    ],
                    ..Default::default()
                }
                .into_node(),
                content,
            ],
            ..Default::default()
        }
        .into_node(),
    )
}
