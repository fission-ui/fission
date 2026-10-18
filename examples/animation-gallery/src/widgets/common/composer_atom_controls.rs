use crate::state::{
    available_composition_atoms_for_path, default_composition_atoms_for_path, motion_atom_label,
    set_composition_atoms, AnimationGalleryState, MotionAtom, SetCompositionAtoms,
};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub(super) ComposerAtomControls<AnimationGalleryState> { path: String, atoms: Vec<MotionAtom> }
    |controls, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let mut undo = controls.atoms.clone();
        undo.pop();
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text("Add atom").size(tokens.typography.font_size_sm).color(style::text_primary()),
                    Wrap(direction: FlexDirection::Row, spacing: tokens.spacing.s, children: [
                        for atom in available_composition_atoms_for_path(&controls.path).iter() {
                            SmallButton(
                                motion_atom_label(*atom).to_string(),
                                cx.bind(SetCompositionAtoms({
                                    let mut next = controls.atoms.clone();
                                    next.push(*atom);
                                    next
                                }), reduce_with!(set_composition_atoms)),
                                key: motion_atom_label(*atom),
                            )
                        }
                    ]),
                    Wrap(
                        direction: FlexDirection::Row,
                        spacing: tokens.spacing.s,
                        children: [
                            SmallButton("Undo Last".to_string(), cx.bind(SetCompositionAtoms(undo), reduce_with!(set_composition_atoms))),
                            SmallButton("Clear".to_string(), cx.bind(SetCompositionAtoms(Vec::new()), reduce_with!(set_composition_atoms))),
                            SmallButton(
                                "Reset Default".to_string(),
                                cx.bind(SetCompositionAtoms(default_composition_atoms_for_path(&controls.path)), reduce_with!(set_composition_atoms)),
                            ),
                            Text(format!("{} atoms", controls.atoms.len())).size(tokens.typography.font_size_sm).color(style::text_muted()),
                        ],
                    ),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface_sunken())
}
