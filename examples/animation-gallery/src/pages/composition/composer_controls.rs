use crate::state::{set_composition_atoms, AnimationGalleryState, MotionAtom, SetCompositionAtoms};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

const AVAILABLE_ATOMS: [MotionAtom; 6] = [
    MotionAtom::FromTop,
    MotionAtom::FromBottom,
    MotionAtom::FromLeft,
    MotionAtom::FromRight,
    MotionAtom::Fade,
    MotionAtom::Scale,
];

fission_component! {
    pub(super) ComposerControls<AnimationGalleryState> |_controls, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let mut undo = state.composition_atoms.clone();
        undo.pop();
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    SectionTitle("Composition Builder".to_string()),
                    Wrap(
                        direction: FlexDirection::Row,
                        spacing: tokens.spacing.s,
                        children: [
                            for atom in AVAILABLE_ATOMS {
                                SmallButton(
                                    add_atom_label(atom).to_string(),
                                    cx.bind(
                                        SetCompositionAtoms({
                                            let mut next = state.composition_atoms.clone();
                                            next.push(atom);
                                            next
                                        }),
                                        reduce_with!(set_composition_atoms),
                                    ),
                                    key: add_atom_label(atom),
                                )
                            },
                        ],
                    ),
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            SmallButton(
                                "Undo Last".to_string(),
                                cx.bind(
                                    SetCompositionAtoms(undo),
                                    reduce_with!(set_composition_atoms),
                                ),
                            ),
                            SmallButton(
                                "Clear".to_string(),
                                cx.bind(
                                    SetCompositionAtoms(Vec::new()),
                                    reduce_with!(set_composition_atoms),
                                ),
                            ),
                            Text(format!("{} atoms", state.composition_atoms.len()))
                                .size(tokens.typography.font_size_sm)
                                .color(style::text_muted()),
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
