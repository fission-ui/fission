use crate::state::{set_composition_atoms, AnimationGalleryState, MotionAtom, SetCompositionAtoms};
use crate::style::{color, BORDER};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const AVAILABLE_ATOMS: [MotionAtom; 6] = [
    MotionAtom::FromTop,
    MotionAtom::FromBottom,
    MotionAtom::FromLeft,
    MotionAtom::FromRight,
    MotionAtom::Fade,
    MotionAtom::Scale,
];

pub(super) struct ComposerControls<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ComposerControls<'_>> for Widget {
    fn from(controls: ComposerControls<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut undo = controls.state.composition_atoms.clone();
        undo.pop();

        let add_buttons = AVAILABLE_ATOMS
            .into_iter()
            .map(|atom| {
                let mut next = controls.state.composition_atoms.clone();
                next.push(atom);
                ui::SmallButton {
                    ctx: controls.ctx,
                    label: add_atom_label(atom),
                    action: SetCompositionAtoms(next),
                    reducer: set_composition_atoms,
                }
                .into()
            })
            .collect();

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle {
                    title: "Composition Builder",
                },
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    children: add_buttons,
                },
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Undo Last",
                            action: SetCompositionAtoms(undo),
                            reducer: set_composition_atoms,
                        },
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Clear",
                            action: SetCompositionAtoms(Vec::new()),
                            reducer: set_composition_atoms,
                        },
                        Text::new(format!("{} atoms", controls.state.composition_atoms.len()))
                            .size(tokens.typography.font_size_sm)
                            .color(crate::style::MUTED),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(color(249, 251, 255, 255))
        .into()
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
