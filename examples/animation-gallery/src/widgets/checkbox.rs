use super::checkbox_preview::CheckboxPreview;
use super::common::*;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::Widget;

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
        semantics_identifier: Some("gallery.checkbox.accept_terms".into()),
        checked: state.accepted,
        label: Some("Accept terms".into()),
        on_toggle: Some(toggle_terms),
        disabled: false,
    }.into(),
    ..Default::default()
}.into()"#;
