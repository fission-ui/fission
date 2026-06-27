use super::common::*;
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, ResetTimeline, TogglePlay,
};
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::motion::{scalar, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::{Checkbox, Column, Text, Widget, WidgetId};

pub const PATH: &str = "/widgets/checkbox";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Checkbox",
    subtitle: "custom",
    glyph: "check",
    tint: SOFT_TEAL,
};

pub struct CheckboxPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<CheckboxPage<'_>> for Widget {
    fn from(page: CheckboxPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: CheckboxPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

struct CheckboxPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<CheckboxPreview<'_>> for Widget {
    fn from(preview: CheckboxPreview<'_>) -> Self {
        let state = preview.state;
        let id = WidgetId::explicit("gallery.real.checkbox");
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
                    Text::new("Real Checkbox widget wrapped in native Motion.")
                        .size(12.0)
                        .into(),
                    Motion {
                        id: WidgetId::explicit("gallery.checkbox.motion"),
                        tracks: if checkbox_scale_enabled(state) {
                            vec![MotionTrack::composite(
                                MotionPropertyId::Scale,
                                MotionStartValue::Explicit(scalar(0.94)),
                                scalar(1.0 + progress * 0.04),
                            )]
                        } else {
                            Vec::new()
                        },
                        child: Checkbox {
                            id: Some(id),
                            checked: preview_active(state),
                            on_toggle: Some(on_toggle),
                            label: Some("Accept motion terms".into()),
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

fn checkbox_scale_enabled(state: &AnimationGalleryState) -> bool {
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
        title: "Checkbox",
        description: "Motion wrapper example for built-in controls without widget-owned motion.",
        motions: STANDARD_MOTIONS,
        slots: &["root", "checkmark"],
        tracks: &["root.scale", "checkmark.opacity"],
        exprs: &[
            "state.checked selects target",
            "scalar(0.94) -> scalar(1.04)",
        ],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic:
            "Checkbox demonstrates native Motion wrapping when a widget does not own a motion enum.",
    }
}

const SOURCE: &str = r#"Motion {
    id: WidgetId::explicit("accept_terms.motion"),
    tracks: vec![MotionTrack::composite(
        MotionPropertyId::Scale,
        MotionStartValue::Explicit(scalar(0.94)),
        scalar(if state.accepted { 1.04 } else { 1.0 }),
    )],
    child: Checkbox {
        id: Some(WidgetId::explicit("accept_terms")),
        checked: state.accepted,
        label: Some("Accept terms".into()),
        on_toggle: Some(toggle_terms),
    }.into(),
    ..Default::default()
}.into()"#;
