use super::common::*;
use crate::state::{
    current_composition_atoms, reset_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    MotionPolicy, ResetTimeline,
};
use crate::style::{MUTED, SOFT_TEAL, SURFACE};
use fission::build::BuildCtxHandle;
use fission::widgets::{Popover, PopoverMotion};
use fission::{Button, ButtonVariant, Column, Container, Text, Widget, WidgetId};

pub const PATH: &str = "/widgets/popover";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Popover",
    subtitle: "4 motions",
    glyph: "pop",
    tint: SOFT_TEAL,
};

pub struct PopoverPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PopoverPage<'_>> for Widget {
    fn from(page: PopoverPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: PopoverPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

struct PopoverPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<PopoverPreview<'_>> for Widget {
    fn from(preview: PopoverPreview<'_>) -> Self {
        let state = preview.state;
        let close = preview
            .ctx
            .bind(ResetTimeline, fission::reduce_with!(reset_timeline));
        PreviewShell {
            child: Column {
                gap: Some(12.0),
                children: vec![
                    Text::new("Real Popover widget anchored to the trigger below.")
                        .size(12.0)
                        .color(MUTED)
                        .into(),
                    Popover {
                        id: WidgetId::explicit("gallery.real.popover"),
                        is_open: preview_active(state),
                        on_toggle: None,
                        on_close: Some(close.clone()),
                        trigger: Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Profile actions").into()),
                            ..Default::default()
                        }
                        .into(),
                        content: Container::new(Column {
                            gap: Some(6.0),
                            children: vec![
                                Text::new("Invite teammate").size(13.0).into(),
                                Text::new("Manage permissions").size(13.0).into(),
                                Text::new("Archive workspace").size(13.0).into(),
                                Button {
                                    width: Some(220.0),
                                    variant: ButtonVariant::Outline,
                                    child: Some(Text::new("Close popover").into()),
                                    on_press: Some(close.clone()),
                                    ..Default::default()
                                }
                                .into(),
                            ],
                            ..Default::default()
                        })
                        .padding_all(14.0)
                        .width(260.0)
                        .border_radius(12.0)
                        .bg(SURFACE)
                        .into(),
                        motion: preview_active(state)
                            .then(|| popover_motion(state))
                            .flatten(),
                    }
                    .into(),
                    if preview_active(state) {
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Close popover preview").into()),
                            on_press: Some(close),
                            ..Default::default()
                        }
                        .into()
                    } else {
                        Text::new("Use the playback control to open it; backdrop and close button dismiss it.")
                            .size(11.0)
                            .color(MUTED)
                            .into()
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn popover_motion(state: &AnimationGalleryState) -> Option<PopoverMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(PopoverMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(PopoverMotion::Default),
        MotionChoice::Fade => Some(PopoverMotion::Fade),
        MotionChoice::Scale => Some(PopoverMotion::Scale),
        MotionChoice::Composition => compose_popover_motion(current_composition_atoms(state)),
        MotionChoice::Directional => Some(PopoverMotion::OriginAwareScale),
    }
}

fn compose_popover_motion(atoms: &[MotionAtom]) -> Option<PopoverMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Fade => Some(PopoverMotion::Fade),
        MotionAtom::Scale => Some(PopoverMotion::Scale),
        MotionAtom::OriginScale => Some(PopoverMotion::OriginAwareScale),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}

fn case() -> GalleryCase {
    GalleryCase {
        title: "Popover",
        description: "Surface presence motion while placement remains normal layout behavior.",
        motions: STANDARD_MOTIONS,
        slots: &["trigger", "surface"],
        tracks: &["surface.opacity", "surface.scale"],
        exprs: &["MotionExpr::Scalar", "anchor rect remains layout data"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Popover motion belongs to the surface; placement remains normal layout data.",
    }
}

const SOURCE: &str = r#"Popover {
    id: WidgetId::explicit("profile_popover"),
    trigger,
    content,
    is_open: state.profile_open,
    motion: Some(PopoverMotion::Fade + PopoverMotion::Scale),
    ..Default::default()
}.into()"#;
