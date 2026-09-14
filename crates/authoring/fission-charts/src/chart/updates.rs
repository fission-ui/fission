//! Tweens series values from their previous data to new data.
//!
//! Each line and bar point gets a scalar motion track that starts from its
//! current value, so when the data changes the motion system eases every point
//! from where it was to where it now belongs. Lowering reads the eased values
//! back before drawing; hit testing keeps the real data.

use super::*;

/// Charts with more points than this update without animating.
const MAX_ANIMATED_POINTS: usize = 400;

fn value_property(series_index: usize, point: usize) -> MotionPropertyId {
    MotionPropertyId::custom(format!("fission_charts::value::{series_index}::{point}"))
}

/// The identity value tracks are registered under, beside the progress track.
fn values_id(chart: &Chart) -> WidgetId {
    WidgetId::derived(chart.root_id().as_u128(), &[0xC4A7_DA7A])
}

fn animates_updates(chart: &Chart) -> bool {
    chart.animation.enabled && chart.animation.animate_updates
}

/// The motion tracks that ease each line and bar value to its current data,
/// when update animation is on and the chart is small enough.
pub(super) fn update_tracks(chart: &Chart) -> Option<MotionDeclaration> {
    if !animates_updates(chart) {
        return None;
    }
    let model = ChartModel::from_chart(chart);
    let series: Vec<(usize, Vec<f32>)> = model
        .series
        .iter()
        .enumerate()
        .filter_map(|(index, series)| match series {
            ResolvedSeries::Line(line) => Some((index, line.values.clone())),
            ResolvedSeries::Bar(bar) => Some((index, bar.values.clone())),
            _ => None,
        })
        .collect();
    let points: usize = series.iter().map(|(_, values)| values.len()).sum();
    if points == 0 || points > MAX_ANIMATED_POINTS {
        return None;
    }
    let transition = MotionTransition::tween(
        chart.animation.duration_ms,
        chart_easing(chart.animation.easing),
    );
    let tracks = series
        .iter()
        .flat_map(|(series_index, values)| {
            let transition = &transition;
            values
                .iter()
                .enumerate()
                .map(move |(point, value)| MotionTrack {
                    property: value_property(*series_index, point),
                    phase: MotionPhase::Composite,
                    from: MotionStartValue::Current,
                    to: scalar(*value),
                    transition: transition.clone(),
                })
        })
        .collect();
    Some(MotionDeclaration {
        id: values_id(chart),
        kind: MotionDeclarationKind::Tracks { tracks },
    })
}

/// Replaces line and bar values with their eased values, when update animation is on.
pub(super) fn apply_animated_values(
    model: &mut ChartModel,
    chart: &Chart,
    cx: &fission_core::internal::LoweringContext,
) {
    if !animates_updates(chart) {
        return;
    }
    let id = values_id(chart);
    let motion = &cx.runtime_state().motion.values;
    for (series_index, series) in model.series.iter_mut().enumerate() {
        let values = match series {
            ResolvedSeries::Line(line) => &mut line.values,
            ResolvedSeries::Bar(bar) => &mut bar.values,
            _ => continue,
        };
        for (point, value) in values.iter_mut().enumerate() {
            if let Some(eased) = motion
                .get(&(id, value_property(series_index, point)))
                .and_then(fission_core::MotionValue::as_scalar_like)
            {
                *value = eased;
            }
        }
    }
}
