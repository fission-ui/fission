use crate::state::GalleryState;
use crate::style::{amber, blue};
use fission::charts::{
    AxisPointer, AxisPointerType, Chart, ChartAnimation, ChartAnimationKind, ChartEmphasis,
    ChartLegendSelectionMode, ChartSelectionMode, ChartTooltipTrigger, MarkLine, MarkPoint,
};
use fission::core::ViewHandle;

mod cartesian;
pub(crate) mod cartesian_variants;
mod chart_selection_empty;
pub(crate) mod components;
pub(crate) mod coordinates;
pub(crate) mod dataset_3d;
pub(crate) mod deep_catalog;
mod deep_catalog_chart;
mod dynamic;
mod relationship_geo;
mod renderable;
mod selected_chart;
mod statistical;

pub(super) use renderable::GalleryBuildExt;
pub(crate) use selected_chart::SelectedChart;

pub(crate) const GALLERY_CHART_HEIGHT: f32 = 520.0;

pub(crate) fn configure_chart(mut chart: Chart, view: ViewHandle<GalleryState>) -> Chart {
    if view.state().interactions {
        let interaction = chart
            .interaction
            .clone()
            .tooltip_trigger(ChartTooltipTrigger::Item)
            .selection_mode(ChartSelectionMode::Single)
            .emit_events(true)
            .keyboard_focus(true)
            .emphasis(ChartEmphasis::data())
            .legend_selection(ChartLegendSelectionMode::Toggle);
        chart = chart.interaction(interaction);
        // Only charts with an x axis get an axis pointer; pies and other radial
        // charts would otherwise show a stray line.
        if chart.x_axis.is_some() && chart.axis_pointer.is_none() {
            chart = chart.axis_pointer(AxisPointer::new().pointer_type(AxisPointerType::Shadow));
        }
    }

    if view.state().animations {
        chart = chart.animation(
            ChartAnimation::enter(ChartAnimationKind::Sweep)
                .duration_ms(1200)
                .stagger_ms(14),
        );
    }

    if view.state().markers {
        chart = chart
            .mark_line(MarkLine::y("target", 160.0 * view.state().data_scale).color(amber()))
            .mark_point(
                MarkPoint::xy("sample", 3.0, 210.0 * view.state().data_scale).color(blue()),
            );
    }

    chart
}
