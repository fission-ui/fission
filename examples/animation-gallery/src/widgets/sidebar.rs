use super::common::*;
use super::sidebar_preview::SidebarPreview;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_VIOLET;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/sidebar";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Sidebar",
    subtitle: "custom",
    glyph: "rail",
    tint: SOFT_VIOLET,
};

pub struct SidebarPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<SidebarPage<'_>> for Widget {
    fn from(page: SidebarPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: SidebarPreview { state: page.state }.into(),
        }
        .into()
    }
}

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

const SOURCE: &str = r#"AppSidebar {
    expanded: view.state().sidebar_expanded,
}.into()

impl From<AppSidebar> for Widget {
    fn from(sidebar: AppSidebar) -> Self {
        Motion {
            id: WidgetId::explicit("app_sidebar.width"),
            tracks: vec![width_track(sidebar.expanded)],
            child: SidebarContent {
                expanded: sidebar.expanded,
            }.into(),
            ..Default::default()
        }.into()
    }
}"#;
