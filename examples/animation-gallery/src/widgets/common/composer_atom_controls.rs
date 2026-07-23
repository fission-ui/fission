use crate::state::{
    available_composition_atoms_for_path, default_composition_atoms_for_path, motion_atom_label,
    set_composition_atoms, AnimationGalleryState, MotionAtom, SetCompositionAtoms,
};
use crate::style::{color, BORDER, MUTED};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub(super) struct ComposerAtomControls<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub path: &'a str,
    pub atoms: &'a [MotionAtom],
}

impl From<ComposerAtomControls<'_>> for Widget {
    fn from(controls: ComposerAtomControls<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let add_buttons = available_composition_atoms_for_path(controls.path)
            .iter()
            .map(|atom| {
                let mut next = controls.atoms.to_vec();
                next.push(*atom);
                ui::SmallButton {
                    ctx: controls.ctx,
                    label: motion_atom_label(*atom),
                    action: SetCompositionAtoms(next),
                    reducer: set_composition_atoms,
                }
                .into()
            })
            .collect();
        let mut undo = controls.atoms.to_vec();
        undo.pop();

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new("Add atom")
                    .size(tokens.typography.font_size_sm)
                    .color(crate::style::INK),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    children: add_buttons,
                },
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
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
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Reset Default",
                            action: SetCompositionAtoms(default_composition_atoms_for_path(
                                controls.path
                            ),),
                            reducer: set_composition_atoms,
                        },
                        Text::new(format!("{} atoms", controls.atoms.len()))
                            .size(tokens.typography.font_size_sm)
                            .color(MUTED),
                    ],
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
