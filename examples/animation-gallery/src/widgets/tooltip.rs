use super::common::*;
use super::tooltip_preview::TooltipPreview;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_BLUE;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/tooltip";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Tooltip",
    subtitle: "4 motions",
    glyph: "tip",
    tint: SOFT_BLUE,
};

pub struct TooltipPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<TooltipPage<'_>> for Widget {
    fn from(page: TooltipPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: TooltipPreview { state: page.state }.into(),
        }
        .into()
    }
}

fn case() -> GalleryCase {
    GalleryCase {
        title: "Tooltip",
        description: "Subtle opt-in surface motion for hover and explicit visibility.",
        motions: STANDARD_MOTIONS,
        slots: &["trigger", "surface"],
        tracks: &["surface.opacity", "surface.translate_y"],
        exprs: &["hover predicate", "px(4) -> px(0)"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic:
            "Tooltip motion is quiet by default and can be disabled without changing the trigger.",
    }
}

const SOURCE: &str = r#"Tooltip {
    id: WidgetId::explicit("save_tip"),
    child: save_button,
    text: "Saved locally".into(),
    is_visible: state.force_tip,
    motion: Some(TooltipMotion::Default),
}.into()"#;
