use super::helpers::*;
use super::specialized::*;
use super::standard_series::*;
use super::*;

pub(super) fn render_series(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let bar_groups = count_bar_groups(&model.series);
    let mut bar_group_index = 0usize;
    let mut bar_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut line_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let animation = ChartAnimationFrame::from_chart(chart, cx);

    for (series_index, series) in model.series.iter().enumerate() {
        match series {
            ResolvedSeries::Bar(bar) => {
                let group_index = if bar.source.stack.is_none() {
                    let idx = bar_group_index;
                    bar_group_index += 1;
                    idx
                } else {
                    0
                };
                render_bar(
                    cx,
                    root,
                    bar,
                    &mut bar_stacks,
                    model,
                    area,
                    &x_scale,
                    &y_scale,
                    theme,
                    group_index,
                    bar_groups,
                    animation,
                    series_index,
                );
            }
            ResolvedSeries::Line(line) => render_line(
                cx,
                root,
                line,
                &mut line_stacks,
                model,
                area,
                &x_scale,
                &y_scale,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Scatter(scatter) => render_scatter(
                cx,
                root,
                &scatter.data,
                scatter.color,
                chart.visual_map.as_ref(),
                area,
                &x_scale,
                &y_scale,
                theme,
                false,
                animation,
                series_index,
            ),
            ResolvedSeries::Bubble(bubble) => render_bubble(
                cx,
                root,
                bubble,
                chart.visual_map.as_ref(),
                area,
                &x_scale,
                &y_scale,
                animation,
                series_index,
            ),
            ResolvedSeries::EffectScatter(effect) => render_scatter(
                cx,
                root,
                &effect.data,
                effect.color,
                chart.visual_map.as_ref(),
                area,
                &x_scale,
                &y_scale,
                theme,
                true,
                animation,
                series_index,
            ),
            ResolvedSeries::Pie(pie) => {
                render_pie(cx, root, pie, area, theme, animation, series_index)
            }
            ResolvedSeries::Boxplot(boxplot) => render_boxplot(
                cx,
                root,
                boxplot,
                model,
                area,
                &y_scale,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Candlestick(candle) => render_candlestick(
                cx,
                root,
                candle,
                model,
                area,
                &y_scale,
                animation,
                series_index,
            ),
            ResolvedSeries::Heatmap(heatmap) => render_heatmap(
                cx,
                root,
                heatmap,
                model,
                chart.visual_map.as_ref(),
                area,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::CalendarHeatmap(calendar) => render_calendar_heatmap(
                cx,
                root,
                calendar,
                chart.visual_map.as_ref(),
                area,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Lines(lines) => {
                render_lines(cx, root, lines, area, theme, animation, series_index)
            }
            ResolvedSeries::Graph(graph) => {
                render_graph(cx, root, graph, area, theme, animation, series_index)
            }
            ResolvedSeries::Tree(tree) => {
                render_tree(cx, root, tree, area, theme, animation, series_index)
            }
            ResolvedSeries::Treemap(treemap) => {
                render_treemap(cx, root, treemap, area, theme, animation, series_index)
            }
            ResolvedSeries::Radar(radar) => {
                render_radar(cx, root, radar, area, theme, animation, series_index)
            }
            ResolvedSeries::Funnel(funnel) => {
                render_funnel(cx, root, funnel, area, theme, animation, series_index)
            }
            ResolvedSeries::Gauge(gauge) => {
                render_gauge(cx, root, gauge, area, theme, animation, series_index)
            }
            ResolvedSeries::Map(map) => render_map(
                cx,
                root,
                map,
                chart.visual_map.as_ref(),
                area,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Sankey(sankey) => {
                render_sankey(cx, root, sankey, area, theme, animation, series_index)
            }
            ResolvedSeries::Parallel(parallel) => {
                render_parallel(cx, root, parallel, area, theme, animation, series_index)
            }
            ResolvedSeries::Sunburst(sunburst) => {
                render_sunburst(cx, root, sunburst, area, theme, animation, series_index)
            }
            ResolvedSeries::ThemeRiver(river) => {
                render_theme_river(cx, root, river, area, theme, animation, series_index)
            }
            ResolvedSeries::PictorialBar(pic) => render_pictorial_bar(
                cx,
                root,
                pic,
                model,
                area,
                &y_scale,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Liquidfill(liquid) => {
                render_liquidfill(cx, root, liquid, area, theme, animation, series_index)
            }
            ResolvedSeries::Wordcloud(words) => {
                render_wordcloud(cx, root, words, area, theme, animation, series_index)
            }
            ResolvedSeries::PolarBar(polar) => {
                render_polar_bar(cx, root, polar, area, theme, animation, series_index)
            }
            ResolvedSeries::PolarLine(polar) => {
                render_polar_line(cx, root, polar, area, theme, animation, series_index)
            }
            ResolvedSeries::SingleAxis(single_axis) => {
                render_single_axis(cx, root, single_axis, area, theme, animation, series_index)
            }
        }
    }
}
