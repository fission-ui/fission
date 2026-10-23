use super::common::*;
use super::drawer_preview::DrawerPreview;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

pub const PATH: &str = "/widgets/drawer";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Drawer",
    subtitle: "5 motions",
    glyph: "panel",
    tint: style::Tint::Primary,
};

fission_component! { pub DrawerPage<AnimationGalleryState> {} |_page, _state, _cx| {} => WidgetPage(case(), DrawerPreview()) }

fn case() -> GalleryCase {
    GalleryCase {
        title: "Drawer",
        description: "Panel and backdrop motion with side-aware directional presets.",
        motions: DIRECTIONAL_MOTIONS,
        slots: &["backdrop", "panel"],
        tracks: &["panel.translate_x", "panel.opacity", "backdrop.opacity"],
        exprs: &["px(side_width) -> px(0)", "scalar(0) -> scalar(1)"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic:
            "Drawer direction is derived from the side and still lowers to ordinary tracks.",
    }
}

const SOURCE: &str = r#"Drawer(
    id: WidgetId::explicit("settings_drawer"),
    side: DrawerSide::Right,
    motion: DrawerMotion::FromSide + DrawerMotion::Fade,
)"#;
