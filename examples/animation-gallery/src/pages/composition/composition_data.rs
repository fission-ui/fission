use crate::state::{motion_atom_label, AnimationGalleryState, MotionAtom, MotionPolicy};

pub(super) fn composition_expression(state: &AnimationGalleryState) -> String {
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

pub(super) fn atom_sequence(state: &AnimationGalleryState) -> String {
    if state.composition_atoms.is_empty() {
        return "empty".into();
    }
    state
        .composition_atoms
        .iter()
        .enumerate()
        .map(|(index, atom)| format!("{}: {}", index + 1, motion_atom_label(*atom)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn lowered_tracks(state: &AnimationGalleryState) -> String {
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

pub(super) fn composition_source(state: &AnimationGalleryState, path: &str) -> String {
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

pub(super) fn policy_summary(policy: MotionPolicy) -> &'static str {
    match policy {
        MotionPolicy::Full => "Full: use composed atoms",
        MotionPolicy::Reduced => "Reduced: fade only",
        MotionPolicy::Disabled => "Disabled: no interpolation",
    }
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
