use super::common::*;
use super::tabs_preview::TabsPreview;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_BLUE;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/tabs";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Tabs",
    subtitle: "4 motions",
    glyph: "tabs",
    tint: SOFT_BLUE,
};

pub struct TabsPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<TabsPage<'_>> for Widget {
    fn from(page: TabsPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: TabsPreview {
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
        title: "Tabs",
        description: "Indicator and active-content motion tied to stable tab slots.",
        motions: STANDARD_MOTIONS,
        slots: &["indicator", "content"],
        tracks: &[
            "indicator.translate_x",
            "indicator.width",
            "content.opacity",
        ],
        exprs: &[
            "MotionExpr::LayoutX(active)",
            "MotionExpr::LayoutWidth(active)",
        ],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic:
            "Tabs demonstrate layout-derived MotionExpr values without exposing internal nodes.",
    }
}

const SOURCE: &str = r#"Tabs {
    active_index: view.state().selected_tab,
    items,
    motion: Some(TabsMotion::Indicator + TabsMotion::SlideContent),
    ..Default::default()
}.into()"#;
