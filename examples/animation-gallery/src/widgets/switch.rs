use super::common::*;
use super::switch_preview::SwitchPreview;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_BLUE;
use fission::build::BuildCtxHandle;
use fission::Widget;

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
        semantics_identifier: Some("gallery.switch.sync".into()),
        checked: state.sync_enabled,
        on_toggle: Some(toggle_sync),
    }.into(),
    ..Default::default()
}.into()"#;
