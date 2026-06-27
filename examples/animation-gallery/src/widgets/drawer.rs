use super::common::*;
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style::{MUTED, SOFT_BLUE};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::widgets::{Drawer, DrawerMotion, DrawerSide};
use fission::{Button, ButtonVariant, Column, Text, Widget, WidgetId};

pub const PATH: &str = "/widgets/drawer";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Drawer",
    subtitle: "5 motions",
    glyph: "panel",
    tint: SOFT_BLUE,
};

pub struct DrawerPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<DrawerPage<'_>> for Widget {
    fn from(page: DrawerPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: DrawerPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

struct DrawerPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<DrawerPreview<'_>> for Widget {
    fn from(preview: DrawerPreview<'_>) -> Self {
        let ctx = preview.ctx;
        let state = preview.state;
        let close = ctx.bind(ResetTimeline, fission::reduce_with!(reset_timeline));
        PreviewShell {
            child: Column {
                gap: Some(12.0),
                children: vec![
                    Text::new(
                        "Real Drawer widget. Play opens the side panel through the portal layer.",
                    )
                    .size(12.0)
                    .color(MUTED)
                    .into(),
                    ui::SmallButton {
                        ctx,
                        label: "Open real drawer",
                        action: TogglePlay,
                        reducer: toggle_play,
                    }
                    .into(),
                    Drawer {
                        id: WidgetId::explicit("gallery.real.drawer"),
                        side: DrawerSide::Right,
                        is_open: preview_active(state),
                        on_dismiss: Some(close.clone()),
                        content: Column {
                            gap: Some(10.0),
                            children: vec![
                                Text::new("Settings").size(18.0).into(),
                                Text::new("This is the actual Drawer content.")
                                    .size(12.0)
                                    .into(),
                                Button {
                                    variant: ButtonVariant::Outline,
                                    child: Some(Text::new("Close drawer").into()),
                                    on_press: Some(close),
                                    ..Default::default()
                                }
                                .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                        width: Some(320.0),
                        motion: preview_active(state)
                            .then(|| drawer_motion(state))
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

fn drawer_motion(state: &AnimationGalleryState) -> Option<DrawerMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(DrawerMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(DrawerMotion::Default),
        MotionChoice::Fade => Some(DrawerMotion::Fade),
        MotionChoice::Directional => Some(DrawerMotion::FromSide),
        MotionChoice::Composition => compose_drawer_motion(current_composition_atoms(state)),
        MotionChoice::Scale => Some(DrawerMotion::Default),
    }
}

fn compose_drawer_motion(atoms: &[MotionAtom]) -> Option<DrawerMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::FromSide => Some(DrawerMotion::FromSide),
        MotionAtom::FromLeft => Some(DrawerMotion::FromLeft),
        MotionAtom::FromRight => Some(DrawerMotion::FromRight),
        MotionAtom::FromTop => Some(DrawerMotion::FromTop),
        MotionAtom::FromBottom => Some(DrawerMotion::FromBottom),
        MotionAtom::Fade => Some(DrawerMotion::Fade),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}

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

const SOURCE: &str = r#"Drawer {
    id: WidgetId::explicit("settings_drawer"),
    side: DrawerSide::Right,
    motion: Some(DrawerMotion::FromSide + DrawerMotion::Fade),
    ..drawer
}.into()"#;
