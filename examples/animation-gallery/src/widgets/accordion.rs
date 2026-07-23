use super::accordion_preview::AccordionPreview;
use super::common::*;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/accordion";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Accordion",
    subtitle: "4 motions",
    glyph: "stack",
    tint: SOFT_TEAL,
};

pub struct AccordionPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<AccordionPage<'_>> for Widget {
    fn from(page: AccordionPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: AccordionPreview {
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
        title: "Accordion",
        description: "Panel height, opacity, and indicator motion for expandable content.",
        motions: STANDARD_MOTIONS,
        slots: &["panel", "indicator"],
        tracks: &["panel.height", "panel.opacity", "indicator.rotation"],
        exprs: &["MotionExpr::IntrinsicHeight", "deg(0) -> deg(90)"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Accordion height motion is layout phase and must be clipped while evaluating.",
    }
}

const SOURCE: &str = r#"Accordion {
    items,
    motion: Some(
        AccordionMotion::Collapse + AccordionMotion::Fade + AccordionMotion::Chevron,
    ),
}.into()"#;
