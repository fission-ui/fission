use super::common::*;
use crate::state::{
    current_composition_atoms, scrub_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    ScrubTimeline,
};
use crate::style::SOFT_BLUE;
use fission::build::BuildCtxHandle;
use fission::widgets::{TabItem, Tabs, TabsMotion};
use fission::{ComponentSize, Text, Widget};

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

struct TabsPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<TabsPreview<'_>> for Widget {
    fn from(preview: TabsPreview<'_>) -> Self {
        let state = preview.state;
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
        PreviewShell {
            child: Tabs {
                active_index: if progress > 0.5 { 1 } else { 0 },
                items: vec![
                    TabItem {
                        title: "API".into(),
                        content: Text::new("Ergonomic motion").into(),
                        on_press: Some(
                            preview
                                .ctx
                                .bind(ScrubTimeline(0), fission::reduce_with!(scrub_timeline)),
                        ),
                    },
                    TabItem {
                        title: "IR".into(),
                        content: Text::new("Lowered MotionExpr").into(),
                        on_press: Some(
                            preview
                                .ctx
                                .bind(ScrubTimeline(300), fission::reduce_with!(scrub_timeline)),
                        ),
                    },
                ],
                size: ComponentSize::Sm,
                motion: match state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Composition => Some(
                        compose_tabs_motion(current_composition_atoms(state))
                            .unwrap_or(TabsMotion::Indicator + TabsMotion::SlideContent),
                    ),
                    _ => Some(TabsMotion::Default),
                },
            }
            .into(),
        }
        .into()
    }
}

fn compose_tabs_motion(atoms: &[MotionAtom]) -> Option<TabsMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Indicator => Some(TabsMotion::Indicator),
        MotionAtom::FadeContent => Some(TabsMotion::FadeContent),
        MotionAtom::SlideContent => Some(TabsMotion::SlideContent),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
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
