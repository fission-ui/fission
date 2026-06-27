use super::common::*;
use crate::state::{current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice};
use crate::style::{BORDER, INK, MUTED, SOFT_VIOLET, SURFACE};
use fission::build::BuildCtxHandle;
use fission::{Column, Container, Row, Text, Widget};

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

struct SidebarPreview<'a> {
    state: &'a AnimationGalleryState,
}

impl From<SidebarPreview<'_>> for Widget {
    fn from(preview: SidebarPreview<'_>) -> Self {
        let state = preview.state;
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
        let width_progress = if state.motion != MotionChoice::Composition
            || current_composition_atoms(state)
                .iter()
                .any(|atom| matches!(atom, MotionAtom::Width))
        {
            progress
        } else {
            0.0
        };
        PreviewShell {
            child: Row {
                gap: Some(12.0),
                children: vec![
                    Container::new(Column {
                        gap: Some(8.0),
                        children: vec![
                            Text::new("Inbox").size(12.0).color(INK).into(),
                            Text::new("Archive").size(12.0).color(MUTED).into(),
                            Text::new("Settings").size(12.0).color(MUTED).into(),
                        ],
                        ..Default::default()
                    })
                    .width(120.0 + 40.0 * width_progress)
                    .height(118.0)
                    .padding_all(14.0)
                    .border_radius(14.0)
                    .border(BORDER, 1.0)
                    .bg(SURFACE)
                    .into(),
                    Text::new("Sidebar uses Drawer-style native motion.")
                        .size(12.0)
                        .color(MUTED)
                        .into(),
                ],
                ..Default::default()
            }
            .into(),
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
            child: sidebar_content(sidebar.expanded),
            ..Default::default()
        }.into()
    }
}"#;
