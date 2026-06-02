use crate::showcase::build_showcase;
use crate::state::{GalleryState, SHOWCASE_CATEGORY};
use crate::style::{amber, blue};
use fission::charts::{
    Chart, ChartAnimation, ChartAnimationKind, ChartSelectionMode, ChartTooltipTrigger, MarkLine,
    MarkPoint,
};
use fission::core::op::Color;
use fission::core::ui::{Container, Text, Widget};
use fission::core::{BuildCtxHandle, ViewHandle};
use fission::three_d::Scene3D;

mod cartesian;
pub(crate) mod cartesian_variants;
pub(crate) mod components;
pub(crate) mod coordinates;
pub(crate) mod dataset_3d;
pub(crate) mod deep_catalog;
mod dynamic;
mod relationship_geo;
mod statistical;

pub(crate) fn build_selected_chart(
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    content_width: f32,
    scale: f32,
) -> Widget {
    let s = scale;
    match (view.state().selected_category, view.state().selected_chart) {
        (SHOWCASE_CATEGORY, 0) => build_showcase(ctx, view, content_width, s),
        (0, chart) => cartesian::build_chart(chart, ctx, view, content_width, s),
        (1, chart) => cartesian_variants::build_chart(chart, ctx, view, content_width, s),
        (2, chart) => statistical::build_chart(chart, ctx, view, content_width, s),
        (3, chart) => relationship_geo::build_chart(chart, ctx, view, content_width, s),
        (4, chart) => dynamic::build_chart(chart, ctx, view, content_width, s),
        (5, chart) => dataset_3d::build_chart(chart, ctx, view, content_width, s),
        (6, chart) => components::build_chart(chart, ctx, view, content_width, s),
        (7, chart) => coordinates::build_chart(chart, ctx, view, content_width, s),
        (category, chart) if category >= deep_catalog::DEEP_CATEGORY_OFFSET => {
            deep_catalog::build_chart(category, chart, ctx, view, content_width, s).unwrap_or_else(
                || {
                    Container::new(Text::new("Select a chart from the gallery").color(Color {
                        r: 150,
                        g: 150,
                        b: 150,
                        a: 255,
                    }))
                    .into()
                },
            )
        }
        _ => Container::new(Text::new("Select a chart from the gallery").color(Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        }))
        .into(),
    }
}

pub(super) trait GalleryBuildExt {
    fn build_in_gallery(
        self,
        ctx: BuildCtxHandle<GalleryState>,
        view: ViewHandle<GalleryState>,
        content_width: f32,
    ) -> Widget;
}

impl GalleryBuildExt for Chart {
    fn build_in_gallery(
        self,
        _ctx: BuildCtxHandle<GalleryState>,
        view: ViewHandle<GalleryState>,
        content_width: f32,
    ) -> Widget {
        configure_chart(
            self,
            view,
            gallery_chart_width(content_width),
            gallery_chart_height(),
        )
        .into()
    }
}

impl GalleryBuildExt for Scene3D {
    fn build_in_gallery(
        self,
        _ctx: BuildCtxHandle<GalleryState>,
        _view: ViewHandle<GalleryState>,
        content_width: f32,
    ) -> Widget {
        self.width(gallery_chart_width(content_width))
            .height(gallery_chart_height())
            .into()
    }
}

pub(crate) fn configure_chart(
    mut chart: Chart,
    view: ViewHandle<GalleryState>,
    width: f32,
    height: f32,
) -> Chart {
    chart = chart.width(width).height(height);

    if view.state().interactions {
        let interaction = chart
            .interaction
            .clone()
            .tooltip_trigger(ChartTooltipTrigger::Item)
            .selection_mode(ChartSelectionMode::Single)
            .emit_events(true)
            .keyboard_focus(true);
        chart = chart.interaction(interaction);
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

fn gallery_chart_width(content_width: f32) -> f32 {
    (content_width - 8.0).clamp(360.0, 1120.0)
}

fn gallery_chart_height() -> f32 {
    520.0
}
