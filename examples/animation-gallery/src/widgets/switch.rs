use super::common::*;
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, ResetTimeline, TogglePlay,
};
use crate::style::SOFT_BLUE;
use fission::build::BuildCtxHandle;
use fission::motion::{scalar, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::{Column, Switch, Text, Widget, WidgetId};

pub const PATH: &str = "/widgets/switch";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Switch",
    subtitle: "custom",
    glyph: "toggle",
    tint: SOFT_BLUE,
};

pub struct SwitchPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<SwitchPage<'_>> for Widget {
    fn from(page: SwitchPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: SwitchPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

struct SwitchPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<SwitchPreview<'_>> for Widget {
    fn from(preview: SwitchPreview<'_>) -> Self {
        let state = preview.state;
        let progress = preview_progress(state);
        let on_toggle = if preview_active(state) {
            preview
                .ctx
                .bind(ResetTimeline, fission::reduce_with!(reset_timeline))
        } else {
            preview
                .ctx
                .bind(TogglePlay, fission::reduce_with!(toggle_play))
        };
        PreviewShell {
            child: Column {
                gap: Some(12.0),
                children: vec![
                    Text::new("Real Switch widget wrapped in native Motion.")
                        .size(12.0)
                        .into(),
                    Motion {
                        id: WidgetId::explicit("gallery.switch.motion"),
                        tracks: if switch_scale_enabled(state) {
                            vec![MotionTrack::composite(
                                MotionPropertyId::Scale,
                                MotionStartValue::Explicit(scalar(0.94)),
                                scalar(1.0 + progress * 0.04),
                            )]
                        } else {
                            Vec::new()
                        },
                        child: Switch {
                            id: Some(WidgetId::explicit("gallery.real.switch")),
                            checked: preview_active(state),
                            on_toggle: Some(on_toggle),
                        }
                        .into(),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn switch_scale_enabled(state: &AnimationGalleryState) -> bool {
    if !policy_allows_motion(state) {
        return false;
    }
    state.motion != MotionChoice::Composition
        || current_composition_atoms(state).iter().any(|atom| {
            matches!(
                atom,
                MotionAtom::Scale | MotionAtom::HoverScale | MotionAtom::PressScale
            )
        })
}

fn case() -> GalleryCase {
    GalleryCase {
        title: "Switch",
        description: "Motion wrapper example for stateful toggle controls.",
        motions: STANDARD_MOTIONS,
        slots: &["track", "thumb"],
        tracks: &["thumb.translate_x", "track.background_color"],
        exprs: &[
            "checked state drives widget layout",
            "root.scale track wraps the switch",
        ],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Switch motion illustrates layout-independent thumb translation.",
    }
}

const SOURCE: &str = r#"Motion {
    id: WidgetId::explicit("sync_switch_motion"),
    tracks: vec![MotionTrack::composite(
        MotionPropertyId::Scale,
        MotionStartValue::Explicit(scalar(0.94)),
        scalar(if state.sync_enabled { 1.04 } else { 1.0 }),
    )],
    child: Switch {
        id: Some(WidgetId::explicit("sync_switch")),
        checked: state.sync_enabled,
        on_toggle: Some(toggle_sync),
    }.into(),
    ..Default::default()
}.into()"#;
