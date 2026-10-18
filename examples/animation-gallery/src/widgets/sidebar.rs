use super::common::*;
use super::sidebar_preview::SidebarPreview;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

pub const PATH: &str = "/widgets/sidebar";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Sidebar",
    subtitle: "custom",
    glyph: "rail",
    tint: style::Tint::Secondary,
};

fission_component! { pub SidebarPage<AnimationGalleryState> {} |_page, _state, _cx| {} => WidgetPage(case(), SidebarPreview()) }

fn case() -> GalleryCase {
    GalleryCase {
        title: "Sidebar",
        description: "Composite custom widget pattern built from Drawer-style motion.",
        motions: DIRECTIONAL_MOTIONS,
        slots: &["rail", "content"],
        tracks: &["rail.width", "content.opacity"],
        exprs: &["MotionExpr::Px", "MotionPhase::Layout"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Sidebar is a composition pattern, not a hidden shell behavior.",
    }
}

const SOURCE: &str = r#"AppSidebar(expanded: state.sidebar_expanded)

fission_component! {
    pub AppSidebar<()> { expanded: bool }
    |sidebar, _state, _cx| {} =>
        Motion(
            id: WidgetId::explicit("app_sidebar.width"),
            tracks: vec![width_track(sidebar.expanded)],
            child: SidebarContent(expanded: sidebar.expanded),
        )
}"#;
