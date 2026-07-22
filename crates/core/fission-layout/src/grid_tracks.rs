use fission_ir::op::GridTrack;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum IntrinsicAxis {
    Min,
    Max,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TrackSizing {
    pub(crate) base: f32,
    pub(crate) limit: f32,
    pub(crate) flex: f32,
    pub(crate) intrinsic: Option<IntrinsicAxis>,
}

impl TrackSizing {
    pub(crate) fn from_track(track: &GridTrack, available: Option<f32>) -> Self {
        match track {
            GridTrack::Points(value) => Self::fixed(*value),
            GridTrack::Percent(value) => available
                .map(|available| Self::fixed(available * *value / 100.0))
                .unwrap_or_else(|| Self::intrinsic(IntrinsicAxis::Max)),
            GridTrack::Fr(flex) => Self {
                base: 0.0,
                limit: f32::INFINITY,
                flex: flex.max(0.0),
                intrinsic: available.is_none().then_some(IntrinsicAxis::Max),
            },
            GridTrack::Auto | GridTrack::MaxContent => Self::intrinsic(IntrinsicAxis::Max),
            GridTrack::MinContent => Self::intrinsic(IntrinsicAxis::Min),
            GridTrack::MinMax(min, max) => {
                let min = Self::from_track(min, available);
                let max = Self::from_track(max, available);
                Self {
                    base: min.base,
                    limit: if max.flex > 0.0 {
                        f32::INFINITY
                    } else {
                        max.limit.max(min.base)
                    },
                    flex: max.flex,
                    intrinsic: min.intrinsic.or(max.intrinsic),
                }
            }
            GridTrack::Repeat { .. } | GridTrack::AutoFit(_) | GridTrack::AutoFill(_) => {
                debug_assert!(false, "grid repetitions must be expanded before sizing");
                Self::intrinsic(IntrinsicAxis::Max)
            }
        }
    }

    pub(crate) fn grow_to(&mut self, value: f32) {
        self.base = value.max(self.base).min(self.limit);
    }

    fn fixed(value: f32) -> Self {
        let value = value.max(0.0);
        Self {
            base: value,
            limit: value,
            flex: 0.0,
            intrinsic: None,
        }
    }

    fn intrinsic(axis: IntrinsicAxis) -> Self {
        Self {
            base: 0.0,
            limit: f32::INFINITY,
            flex: 0.0,
            intrinsic: Some(axis),
        }
    }
}

pub(crate) fn expand_tracks(
    tracks: &[GridTrack],
    available: Option<f32>,
    gap: f32,
    child_count: usize,
) -> Vec<GridTrack> {
    let mut expanded = Vec::new();
    for track in tracks {
        match track {
            GridTrack::Repeat { count, tracks } => {
                let nested = expand_tracks(tracks, available, gap, child_count);
                for _ in 0..*count {
                    expanded.extend(nested.iter().cloned());
                }
            }
            GridTrack::AutoFit(track) => {
                let minimum = repeat_minimum(track, available);
                let capacity = if minimum > 0.0 {
                    available
                        .map(|available| ((available + gap) / (minimum + gap)).floor() as usize)
                        .unwrap_or(child_count.max(1))
                        .max(1)
                } else {
                    child_count.max(1)
                };
                let count = capacity.min(child_count.max(1));
                expanded.extend(std::iter::repeat_n(track.as_ref().clone(), count));
            }
            GridTrack::AutoFill(track) => {
                let minimum = repeat_minimum(track, available);
                let count = if minimum > 0.0 {
                    available
                        .map(|available| ((available + gap) / (minimum + gap)).floor() as usize)
                        .unwrap_or(child_count.max(1))
                        .max(1)
                } else {
                    child_count.max(1)
                };
                expanded.extend(std::iter::repeat_n(track.as_ref().clone(), count));
            }
            track => expanded.push(track.clone()),
        }
    }
    expanded
}

pub(crate) fn distribute_deficit(tracks: &mut [TrackSizing], start: usize, span: usize, need: f32) {
    if span == 0 || start >= tracks.len() {
        return;
    }
    let end = start.saturating_add(span).min(tracks.len());
    let current = tracks[start..end]
        .iter()
        .map(|track| track.base)
        .sum::<f32>();
    let mut deficit = (need - current).max(0.0);
    while deficit > 0.01 {
        let growable = tracks[start..end]
            .iter()
            .filter(|track| track.base + 0.01 < track.limit)
            .count();
        if growable == 0 {
            break;
        }
        let share = deficit / growable as f32;
        let mut consumed = 0.0;
        for track in &mut tracks[start..end] {
            if track.base + 0.01 >= track.limit {
                continue;
            }
            let growth = share.min(track.limit - track.base);
            track.base += growth;
            consumed += growth;
        }
        if consumed <= 0.01 {
            break;
        }
        deficit -= consumed;
    }
}

pub(crate) fn distribute_flex(tracks: &mut [TrackSizing], available: f32, gap: f32) {
    let gaps = gap * tracks.len().saturating_sub(1) as f32;
    let used = tracks.iter().map(|track| track.base).sum::<f32>() + gaps;
    let remaining = (available - used).max(0.0);
    let total_flex = tracks.iter().map(|track| track.flex).sum::<f32>();
    if remaining <= 0.0 || total_flex <= 0.0 {
        return;
    }
    for track in tracks {
        if track.flex > 0.0 {
            track.grow_to(track.base + remaining * track.flex / total_flex);
        }
    }
}

fn repeat_minimum(track: &GridTrack, available: Option<f32>) -> f32 {
    match track {
        GridTrack::Points(value) => *value,
        GridTrack::Percent(value) => available.map_or(0.0, |size| size * *value / 100.0),
        GridTrack::MinMax(min, _) => repeat_minimum(min, available),
        GridTrack::Repeat { tracks, .. } => tracks
            .iter()
            .map(|track| repeat_minimum(track, available))
            .sum(),
        GridTrack::AutoFit(track) | GridTrack::AutoFill(track) => repeat_minimum(track, available),
        GridTrack::Fr(_) | GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{distribute_flex, expand_tracks, TrackSizing};
    use fission_ir::op::GridTrack;

    #[test]
    fn expands_repeat_and_auto_fit_tracks() {
        let tracks = vec![
            GridTrack::repeat(2, vec![GridTrack::Points(20.0)]),
            GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(100.0),
                GridTrack::Fr(1.0),
            )),
        ];

        let expanded = expand_tracks(&tracks, Some(450.0), 10.0, 3);

        assert_eq!(expanded.len(), 5);
    }

    #[test]
    fn distributes_remaining_space_between_fractional_tracks() {
        let mut tracks = vec![
            TrackSizing::from_track(&GridTrack::Fr(1.0), Some(300.0)),
            TrackSizing::from_track(&GridTrack::Fr(2.0), Some(300.0)),
        ];

        distribute_flex(&mut tracks, 300.0, 0.0);

        assert_eq!(tracks[0].base, 100.0);
        assert_eq!(tracks[1].base, 200.0);
    }
}
