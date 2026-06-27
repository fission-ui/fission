use super::common::*;
use crate::state::{
    current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice, MotionPolicy,
};
use crate::style::{MUTED, SOFT_BLUE};
use fission::build::BuildCtxHandle;
use fission::widgets::{Tooltip, TooltipMotion};
use fission::{Button, ButtonVariant, Column, Text, Widget, WidgetId};

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

struct TooltipPreview<'a> {
    state: &'a AnimationGalleryState,
}

impl From<TooltipPreview<'_>> for Widget {
    fn from(preview: TooltipPreview<'_>) -> Self {
        let state = preview.state;
        PreviewShell {
            child: Column {
                gap: Some(12.0),
                children: vec![
                    Text::new("Real Tooltip widget. Play forces it visible; hover also works.")
                        .size(12.0)
                        .color(MUTED)
                        .into(),
                    Tooltip {
                        id: WidgetId::explicit("gallery.real.tooltip"),
                        child: Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Save").into()),
                            ..Default::default()
                        }
                        .into(),
                        text: "Saved locally with deterministic motion.".into(),
                        is_visible: preview_active(state),
                        motion: preview_active(state)
                            .then(|| tooltip_motion(state))
                            .flatten(),
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn tooltip_motion(state: &AnimationGalleryState) -> Option<TooltipMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(TooltipMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(TooltipMotion::Default),
        MotionChoice::Fade => Some(TooltipMotion::Fade),
        MotionChoice::Scale => Some(TooltipMotion::Scale),
        MotionChoice::Composition => compose_tooltip_motion(current_composition_atoms(state)),
        MotionChoice::Directional => Some(TooltipMotion::FadeAndSlide),
    }
}

fn compose_tooltip_motion(atoms: &[MotionAtom]) -> Option<TooltipMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Fade => Some(TooltipMotion::Fade),
        MotionAtom::Scale => Some(TooltipMotion::Scale),
        MotionAtom::FromTop => Some(TooltipMotion::FadeAndSlide),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
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
