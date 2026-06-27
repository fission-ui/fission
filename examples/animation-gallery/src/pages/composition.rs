use crate::state::{
    motion_atom_label, reset_timeline, set_composition_atoms, AnimationGalleryState, MotionAtom,
    MotionPolicy, ResetTimeline, SetCompositionAtoms,
};
use crate::style::*;
use crate::ui;
use crate::widgets::common::{preview_active, PolicyControl, PreviewShell, TimelineControl};
use fission::build::BuildCtxHandle;
use fission::widgets::{Modal, ModalAction, ModalMotion};
use fission::{Column, Container, FlexDirection, Row, Scroll, Text, Widget, WidgetId};

pub struct CompositionPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub path: String,
}

impl From<CompositionPage<'_>> for Widget {
    fn from(page: CompositionPage<'_>) -> Self {
        let expression = composition_expression(page.state);
        let lowered = lowered_tracks(page.state);
        let source = composition_source(page.state, &page.path);

        Column {
            gap: Some(14.0),
            children: vec![
                ui::PageHeader {
                    title: "Composition Builder",
                    subtitle: "Add any number of widget-owned motion atoms in order, play the real widget, and inspect last-wins lowering.",
                }
                .into(),
                Container::new(Column {
                    gap: Some(14.0),
                    children: vec![
                        ComposerControls {
                            ctx: &page.ctx,
                            state: page.state,
                        }
                        .into(),
                        ExpressionPanel {
                            expression: &expression,
                        }
                        .into(),
                        Row {
                            gap: Some(14.0),
                            children: vec![
                                AtomCard {
                                    title: "Ordered Atoms",
                                    body: &atom_sequence(page.state),
                                    tint: BLUE,
                                }
                                .into(),
                                AtomCard {
                                    title: "Lowered Tracks",
                                    body: &lowered,
                                    tint: VIOLET,
                                }
                                .into(),
                                AtomCard {
                                    title: "Policy",
                                    body: policy_summary(page.state.policy),
                                    tint: TEAL,
                                }
                                .into(),
                                AtomCard {
                                    title: "Conflict Rule",
                                    body: "same slot/property/phase: later wins",
                                    tint: CYAN,
                                }
                                .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                        ui::PageNote {
                            title: "Composition behavior",
                            body: "Add atoms in any order and repeat atoms as often as needed. Different tracks are retained. If two atoms write the same stable slot/property/phase, the later atom wins.",
                        }
                        .into(),
                        CompositionPreview {
                            ctx: &page.ctx,
                            state: page.state,
                        }
                        .into(),
                        ui::CodeBlock { source: &source }.into(),
                        PlaybackControls {
                            ctx: &page.ctx,
                            state: page.state,
                        }
                        .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(16.0)
                .border(BORDER, 1.0)
                .border_radius(16.0)
                .bg(SURFACE)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct ExpressionPanel<'a> {
    expression: &'a str,
}

impl From<ExpressionPanel<'_>> for Widget {
    fn from(panel: ExpressionPanel<'_>) -> Self {
        Container::new(Column {
            gap: Some(6.0),
            children: vec![
                Text::new("Expression").size(11.0).color(MUTED).into(),
                Text::new(panel.expression)
                    .size(11.0)
                    .line_height(15.0)
                    .color(INK)
                    .max_width(820.0)
                    .flex_shrink(1.0)
                    .into(),
            ],
            ..Default::default()
        })
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(12.0)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}

struct ComposerControls<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<ComposerControls<'_>> for Widget {
    fn from(controls: ComposerControls<'_>) -> Self {
        let mut add_buttons = Vec::new();
        for atom in [
            MotionAtom::FromTop,
            MotionAtom::FromBottom,
            MotionAtom::FromLeft,
            MotionAtom::FromRight,
            MotionAtom::Fade,
            MotionAtom::Scale,
        ] {
            let mut next = controls.state.composition_atoms.clone();
            next.push(atom);
            add_buttons.push(
                ui::SmallButton {
                    ctx: controls.ctx,
                    label: add_atom_label(atom),
                    action: SetCompositionAtoms(next),
                    reducer: set_composition_atoms,
                }
                .into(),
            );
        }
        let mut undo = controls.state.composition_atoms.clone();
        undo.pop();

        Container::new(Column {
            gap: Some(10.0),
            children: vec![
                ui::SectionTitle {
                    title: "Composition Builder",
                }
                .into(),
                Row {
                    gap: Some(8.0),
                    children: add_buttons,
                    ..Default::default()
                }
                .into(),
                Row {
                    gap: Some(8.0),
                    children: vec![
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Undo Last",
                            action: SetCompositionAtoms(undo),
                            reducer: set_composition_atoms,
                        }
                        .into(),
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Clear",
                            action: SetCompositionAtoms(Vec::new()),
                            reducer: set_composition_atoms,
                        }
                        .into(),
                        Text::new(format!("{} atoms", controls.state.composition_atoms.len()))
                            .size(11.0)
                            .color(MUTED)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(14.0)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}

struct PlaybackControls<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<PlaybackControls<'_>> for Widget {
    fn from(controls: PlaybackControls<'_>) -> Self {
        Container::new(Column {
            gap: Some(10.0),
            children: vec![
                ui::SectionTitle { title: "Playback" }.into(),
                Row {
                    gap: Some(8.0),
                    children: vec![
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: if controls.state.playing {
                                "Pause"
                            } else {
                                "Play"
                            },
                            action: crate::state::TogglePlay,
                            reducer: crate::state::toggle_play,
                        }
                        .into(),
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Reset",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        }
                        .into(),
                        TimelineControl {
                            ctx: controls.ctx,
                            state: controls.state,
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                PolicyControl {
                    ctx: controls.ctx,
                    state: controls.state,
                }
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(14.0)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}

struct CompositionPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<CompositionPreview<'_>> for Widget {
    fn from(preview: CompositionPreview<'_>) -> Self {
        let close = preview
            .ctx
            .bind(ResetTimeline, fission::reduce_with!(reset_timeline));

        PreviewShell {
            child: Column {
                gap: Some(10.0),
                children: vec![
                    Text::new(
                        "Real Modal widget using the ordered expression. Use playback to open it.",
                    )
                    .size(12.0)
                    .color(MUTED)
                    .into(),
                    Modal {
                        id: WidgetId::explicit("gallery.composition.modal"),
                        title: "Composed motion".into(),
                        content: Text::new("This modal is using the selected composition.").into(),
                        is_open: preview_active(preview.state),
                        on_dismiss: Some(close.clone()),
                        actions: vec![ModalAction {
                            label: "Close".into(),
                            on_press: Some(close),
                            is_primary: true,
                        }],
                        width: Some(400.0),
                        motion: composed_modal_motion(preview.state),
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

fn composed_modal_motion(state: &AnimationGalleryState) -> Option<ModalMotion> {
    if !preview_active(state)
        || state.composition_atoms.is_empty()
        || state.policy == MotionPolicy::Disabled
    {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(ModalMotion::Fade);
    }

    let mut atoms = state.composition_atoms.iter().copied().map(atom_motion);
    let first = atoms.next()?;
    Some(atoms.fold(first, |acc, atom| acc + atom))
}

fn atom_motion(atom: MotionAtom) -> ModalMotion {
    match atom {
        MotionAtom::FromTop => ModalMotion::FromTop,
        MotionAtom::FromBottom => ModalMotion::FromBottom,
        MotionAtom::FromLeft => ModalMotion::FromLeft,
        MotionAtom::FromRight => ModalMotion::FromRight,
        MotionAtom::FromSide => ModalMotion::FromRight,
        MotionAtom::Fade => ModalMotion::Fade,
        MotionAtom::Scale | MotionAtom::OriginScale | MotionAtom::Pop => ModalMotion::Scale,
        MotionAtom::Collapse
        | MotionAtom::Chevron
        | MotionAtom::Indicator
        | MotionAtom::FadeContent
        | MotionAtom::SlideContent
        | MotionAtom::HoverScale
        | MotionAtom::PressScale
        | MotionAtom::Ripple
        | MotionAtom::Width => ModalMotion::Fade,
    }
}

fn composition_expression(state: &AnimationGalleryState) -> String {
    if state.composition_atoms.is_empty() {
        return "None".into();
    }
    state
        .composition_atoms
        .iter()
        .map(|atom| format!("ModalMotion::{}", motion_atom_label(*atom)))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn atom_sequence(state: &AnimationGalleryState) -> String {
    if state.composition_atoms.is_empty() {
        return "empty".into();
    }
    state
        .composition_atoms
        .iter()
        .enumerate()
        .map(|(idx, atom)| format!("{}: {}", idx + 1, motion_atom_label(*atom)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn lowered_tracks(state: &AnimationGalleryState) -> String {
    if state.composition_atoms.is_empty() {
        return "no tracks".into();
    }
    let mut lowered: Vec<(&'static str, &'static str)> = Vec::new();
    for atom in &state.composition_atoms {
        match atom {
            MotionAtom::FromTop => set_lowered(
                &mut lowered,
                "surface.translate_y",
                "surface.translate_y from top",
            ),
            MotionAtom::FromBottom => set_lowered(
                &mut lowered,
                "surface.translate_y",
                "surface.translate_y from bottom",
            ),
            MotionAtom::FromLeft => set_lowered(
                &mut lowered,
                "surface.translate_x",
                "surface.translate_x from left",
            ),
            MotionAtom::FromRight => set_lowered(
                &mut lowered,
                "surface.translate_x",
                "surface.translate_x from right",
            ),
            MotionAtom::FromSide => set_lowered(
                &mut lowered,
                "surface.translate_x",
                "surface.translate_x from side",
            ),
            MotionAtom::Fade => {
                set_lowered(&mut lowered, "backdrop.opacity", "backdrop.opacity");
                set_lowered(&mut lowered, "surface.opacity", "surface.opacity");
            }
            MotionAtom::Scale | MotionAtom::OriginScale | MotionAtom::Pop => {
                set_lowered(&mut lowered, "surface.scale", "surface.scale")
            }
            MotionAtom::Collapse => set_lowered(&mut lowered, "panel.height", "panel.height"),
            MotionAtom::Chevron => {
                set_lowered(&mut lowered, "indicator.rotation", "indicator.rotation")
            }
            MotionAtom::Indicator => set_lowered(
                &mut lowered,
                "indicator.translate_x",
                "indicator.translate_x",
            ),
            MotionAtom::FadeContent => {
                set_lowered(&mut lowered, "content.opacity", "content.opacity")
            }
            MotionAtom::SlideContent => {
                set_lowered(&mut lowered, "content.translate_x", "content.translate_x")
            }
            MotionAtom::HoverScale | MotionAtom::PressScale => {
                set_lowered(&mut lowered, "root.scale", "root.scale")
            }
            MotionAtom::Ripple => set_lowered(&mut lowered, "ripple.spawn", "ripple.spawn"),
            MotionAtom::Width => set_lowered(&mut lowered, "rail.width", "rail.width"),
        }
    }
    lowered
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>()
        .join("\n")
}

fn set_lowered(
    lowered: &mut Vec<(&'static str, &'static str)>,
    key: &'static str,
    value: &'static str,
) {
    if let Some(existing) = lowered
        .iter_mut()
        .find(|(existing_key, _)| *existing_key == key)
    {
        existing.1 = value;
    } else {
        lowered.push((key, value));
    }
}

fn add_atom_label(atom: MotionAtom) -> &'static str {
    match atom {
        MotionAtom::FromTop => "Add FromTop",
        MotionAtom::FromBottom => "Add FromBottom",
        MotionAtom::FromLeft => "Add FromLeft",
        MotionAtom::FromRight => "Add FromRight",
        MotionAtom::FromSide => "Add FromSide",
        MotionAtom::Fade => "Add Fade",
        MotionAtom::Scale => "Add Scale",
        MotionAtom::OriginScale => "Add OriginScale",
        MotionAtom::Pop => "Add Pop",
        MotionAtom::Collapse => "Add Collapse",
        MotionAtom::Chevron => "Add Chevron",
        MotionAtom::Indicator => "Add Indicator",
        MotionAtom::FadeContent => "Add FadeContent",
        MotionAtom::SlideContent => "Add SlideContent",
        MotionAtom::HoverScale => "Add HoverScale",
        MotionAtom::PressScale => "Add PressScale",
        MotionAtom::Ripple => "Add Ripple",
        MotionAtom::Width => "Add Width",
    }
}

fn composition_source(state: &AnimationGalleryState, path: &str) -> String {
    let note = if matches!(path, "/composition/conflict" | "/composition/last-wins") {
        "\n\n// Later atoms targeting the same modal slot/property/phase win."
    } else {
        ""
    };
    format!(
        "Modal {{\n    id: WidgetId::explicit(\"gallery_modal\"),\n    motion: Some({}),\n    ..modal\n}}.into(){}",
        composition_expression(state),
        note
    )
}

fn policy_summary(policy: MotionPolicy) -> &'static str {
    match policy {
        MotionPolicy::Full => "Full: use composed atoms",
        MotionPolicy::Reduced => "Reduced: fade only",
        MotionPolicy::Disabled => "Disabled: no interpolation",
    }
}

struct AtomCard<'a> {
    title: &'a str,
    body: &'a str,
    tint: fission::op::Color,
}

impl From<AtomCard<'_>> for Widget {
    fn from(card: AtomCard<'_>) -> Self {
        Container::new(Column {
            gap: Some(7.0),
            children: vec![
                Text::new(card.title).size(12.0).color(INK).into(),
                Scroll {
                    direction: FlexDirection::Column,
                    height: Some(116.0),
                    show_scrollbar: false,
                    child: Some(Text::new(card.body).size(10.0).color(MUTED).into()),
                    ..Default::default()
                }
                .into(),
                Container::new(Text::new(" "))
                    .height(26.0)
                    .border_radius(8.0)
                    .bg(card.tint)
                    .into(),
            ],
            ..Default::default()
        })
        .width(170.0)
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(12.0)
        .bg(SURFACE)
        .into()
    }
}
