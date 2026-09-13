//! Funnel, liquid fill and word cloud series.

use super::*;

pub(super) fn render_funnel(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    funnel: &crate::series::funnel::FunnelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if funnel.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let max = funnel.data.iter().map(|(_, v)| *v).fold(1.0_f32, f32::max);
    let step_h = area.plot.height() / funnel.data.len() as f32;
    let cx_mid = area.plot.x() + area.plot.width() / 2.0;
    for (idx, (label, value)) in funnel.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let y = area.plot.y() + idx as f32 * step_h;
        let top_w = if idx == 0 {
            area.plot.width()
        } else {
            area.plot.width() * funnel.data[idx - 1].1 / max
        } * item_progress;
        let bot_w = area.plot.width() * *value / max * item_progress;
        let path = format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            cx_mid - top_w / 2.0,
            y,
            cx_mid + top_w / 2.0,
            y,
            cx_mid + bot_w / 2.0,
            y + step_h,
            cx_mid - bot_w / 2.0,
            y + step_h
        );
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(fade_color(
                theme.palette[idx % theme.palette.len()],
                item_progress,
            ))),
            Some(fade_stroke(stroke(Color::WHITE, 1.5), item_progress)),
        );
        if item_progress > 0.82 {
            add_text(
                cx,
                root,
                label,
                12.0,
                Color::WHITE,
                cx_mid - 50.0,
                y + step_h / 2.0 - 8.0,
                100.0,
                16.0,
            );
        }
    }
}

pub(super) fn render_liquidfill(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    liquid: &crate::series::liquidfill::LiquidfillSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    let value = liquid.data.first().copied().unwrap_or(0.0).clamp(0.0, 1.0) * series_progress;
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.34;
    add_rect(
        cx,
        root,
        LayoutRect::new(center.0 - r, center.1 - r, r * 2.0, r * 2.0),
        color(232, 244, 255, 255),
        Some(stroke(filled(liquid.color), 2.0)),
        r,
    );
    let water_y = center.1 + r - value * r * 2.0;
    let path = format!(
        "M {} {} C {} {} {} {} {} {} L {} {} L {} {} Z",
        center.0 - r,
        water_y,
        center.0 - r * 0.45,
        water_y - 16.0,
        center.0 + r * 0.45,
        water_y + 16.0,
        center.0 + r,
        water_y,
        center.0 + r,
        center.1 + r,
        center.0 - r,
        center.1 + r
    );
    add_path(
        cx,
        root,
        &path,
        Some(Fill::Solid(fade_color(
            filled(liquid.color).with_alpha(190),
            series_progress,
        ))),
        None,
    );
    add_text(
        cx,
        root,
        &format!("{:.0}%", value * 100.0),
        24.0,
        theme.title,
        center.0 - 40.0,
        center.1 - 14.0,
        80.0,
        28.0,
    );
}

pub(super) fn render_wordcloud(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    wordcloud: &crate::series::wordcloud::WordcloudSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let layout = crate::layout::wordcloud::WordcloudLayout::compute(
        &wordcloud.data,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (word, size, x, y)) in layout.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        add_text(
            cx,
            root,
            word,
            (*size * (0.78 + item_progress * 0.22)).max(1.0),
            fade_color(theme.palette[idx % theme.palette.len()], item_progress),
            area.plot.x() + x + (*size * (1.0 - item_progress) * 0.08),
            area.plot.y() + y,
            180.0,
            size + 8.0,
        );
    }
}
