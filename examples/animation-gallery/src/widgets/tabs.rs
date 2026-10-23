use super::common::*;
use super::tabs_preview::TabsPreview;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

pub const PATH: &str = "/widgets/tabs";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Tabs",
    subtitle: "4 motions",
    glyph: "tabs",
    tint: style::Tint::Primary,
};

fission_component! { pub TabsPage<AnimationGalleryState> {} |_page, _state, _cx| {} => WidgetPage(case(), TabsPreview()) }

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

const SOURCE: &str = r#"Tabs(
    active_index: view.state().selected_tab,
    items,
    motion: TabsMotion::Indicator + TabsMotion::SlideContent,
)"#;
