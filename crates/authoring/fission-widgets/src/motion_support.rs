use fission_core::motion::{
    dedupe_tracks_later_wins, px, reverse_tracks_for_exit, scalar, MotionEasing, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition,
};
use fission_core::WidgetId;

pub(crate) const SLOT_BACKDROP: u32 = 0xBACC_DA7A;
pub(crate) const SLOT_SURFACE: u32 = 0x5AFA_CE;
pub(crate) const SLOT_PANEL: u32 = 0xCAFE_2A1;
pub(crate) const SLOT_INDICATOR: u32 = 0x1D1_CA70;
pub(crate) const SLOT_CONTENT: u32 = 0xC017_E17;

pub(crate) fn slot_id(parent: WidgetId, slot: u32) -> WidgetId {
    WidgetId::derived(parent.as_u128(), &[slot])
}

pub(crate) fn fade_in(duration_ms: u64) -> MotionTrack {
    MotionTrack::composite(
        MotionPropertyId::Opacity,
        MotionStartValue::Explicit(scalar(0.0)),
        scalar(1.0),
    )
    .transition(MotionTransition::tween(duration_ms, MotionEasing::EaseOut))
}

pub(crate) fn scale_in(from: f32, _duration_ms: u64) -> MotionTrack {
    MotionTrack::composite(
        MotionPropertyId::Scale,
        MotionStartValue::Explicit(scalar(from)),
        scalar(1.0),
    )
    .transition(MotionTransition::spring(360.0, 28.0))
}

pub(crate) fn slide_x_in(offset: f32, duration_ms: u64) -> MotionTrack {
    MotionTrack::composite(
        MotionPropertyId::TranslateX,
        MotionStartValue::Explicit(px(offset)),
        px(0.0),
    )
    .transition(MotionTransition::tween(duration_ms, MotionEasing::EaseOut))
}

pub(crate) fn slide_y_in(offset: f32, duration_ms: u64) -> MotionTrack {
    MotionTrack::composite(
        MotionPropertyId::TranslateY,
        MotionStartValue::Explicit(px(offset)),
        px(0.0),
    )
    .transition(MotionTransition::tween(duration_ms, MotionEasing::EaseOut))
}

pub(crate) fn collapse_y_in(duration_ms: u64) -> MotionTrack {
    MotionTrack {
        property: MotionPropertyId::Height,
        phase: fission_core::motion::MotionPhase::Layout,
        from: MotionStartValue::Explicit(px(0.0)),
        to: fission_core::motion::MotionExpr::IntrinsicHeight,
        transition: MotionTransition::tween(duration_ms, MotionEasing::EaseOut),
    }
}

pub(crate) fn dedupe(tracks: Vec<MotionTrack>) -> Vec<MotionTrack> {
    dedupe_tracks_later_wins(tracks)
}

pub(crate) fn exit_for(enter: &[MotionTrack]) -> Vec<MotionTrack> {
    reverse_tracks_for_exit(enter)
}

pub(crate) fn push_enter_with_exit(
    enter: &mut Vec<MotionTrack>,
    exit: &mut Vec<MotionTrack>,
    track: MotionTrack,
) {
    exit.extend(exit_for(std::slice::from_ref(&track)));
    enter.push(track);
}
