use super::common::*;
use crate::state::{
    current_composition_atoms, toggle_play, AnimationGalleryState, MotionAtom, MotionChoice,
    TogglePlay,
};
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::widgets::{Accordion, AccordionItem, AccordionMotion};
use fission::{Text, Widget};

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

struct AccordionPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<AccordionPreview<'_>> for Widget {
    fn from(preview: AccordionPreview<'_>) -> Self {
        let state = preview.state;
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
        PreviewShell {
            child: Accordion {
                items: vec![AccordionItem {
                    title: "Motion details".into(),
                    content: Text::new(
                        "Panel height, opacity, and indicator rotation are inspectable.",
                    )
                    .into(),
                    is_expanded: progress > 0.2,
                    on_toggle: Some(
                        preview
                            .ctx
                            .bind(TogglePlay, fission::reduce_with!(toggle_play)),
                    ),
                }],
                motion: match state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Composition => Some(
                        compose_accordion_motion(current_composition_atoms(state))
                            .unwrap_or(AccordionMotion::Default),
                    ),
                    _ => Some(AccordionMotion::Default),
                },
            }
            .into(),
        }
        .into()
    }
}

fn compose_accordion_motion(atoms: &[MotionAtom]) -> Option<AccordionMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Collapse => Some(AccordionMotion::Collapse),
        MotionAtom::Fade => Some(AccordionMotion::Fade),
        MotionAtom::Chevron => Some(AccordionMotion::Chevron),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
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
