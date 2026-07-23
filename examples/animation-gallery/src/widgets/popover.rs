use super::common::*;
use super::popover_preview::PopoverPreview;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/popover";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Popover",
    subtitle: "4 motions",
    glyph: "pop",
    tint: SOFT_TEAL,
};

pub struct PopoverPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PopoverPage<'_>> for Widget {
    fn from(page: PopoverPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: PopoverPreview {
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
        title: "Popover",
        description: "Surface presence motion while placement remains normal layout behavior.",
        motions: STANDARD_MOTIONS,
        slots: &["trigger", "surface"],
        tracks: &["surface.opacity", "surface.scale"],
        exprs: &["MotionExpr::Scalar", "anchor rect remains layout data"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Popover motion belongs to the surface; placement remains normal layout data.",
    }
}

const SOURCE: &str = r#"Popover {
    id: WidgetId::explicit("profile_popover"),
    trigger,
    content,
    is_open: state.profile_open,
    motion: Some(PopoverMotion::Fade + PopoverMotion::Scale),
    ..Default::default()
}.into()"#;
