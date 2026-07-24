use crate::state::{composition_type_name_for_path, motion_atom_label, MotionAtom};

pub(super) fn atom_sequence(atoms: &[MotionAtom]) -> String {
    if atoms.is_empty() {
        return "empty".into();
    }
    atoms
        .iter()
        .enumerate()
        .map(|(index, atom)| format!("{}: {}", index + 1, motion_atom_label(*atom)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn composition_expression(path: &str, atoms: &[MotionAtom]) -> String {
    if atoms.is_empty() {
        return "None".into();
    }
    atoms
        .iter()
        .map(|atom| atom_expression(path, *atom))
        .collect::<Vec<_>>()
        .join(" + ")
}

pub(super) fn lowered_composition_tracks(path: &str, atoms: &[MotionAtom]) -> String {
    if atoms.is_empty() {
        return "no tracks".into();
    }
    let mut lowered: Vec<(&'static str, &'static str)> = Vec::new();
    for atom in atoms {
        for (key, value) in atom_tracks(path, *atom) {
            if let Some(existing) = lowered
                .iter_mut()
                .find(|(existing_key, _)| *existing_key == key)
            {
                existing.1 = value;
            } else {
                lowered.push((key, value));
            }
        }
    }
    lowered
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>()
        .join("\n")
}

fn atom_expression(path: &str, atom: MotionAtom) -> String {
    let ty = composition_type_name_for_path(path);
    let name = match (path, atom) {
        ("/widgets/toast", MotionAtom::FromTop) => "SlideFromTop",
        ("/widgets/toast", MotionAtom::FromBottom) => "SlideFromBottom",
        ("/widgets/toast", MotionAtom::Pop) => "Pop",
        ("/widgets/tooltip", MotionAtom::FromTop) => "FadeAndSlide",
        ("/widgets/popover", MotionAtom::OriginScale) => "OriginAwareScale",
        (_, MotionAtom::FadeContent) => "FadeContent",
        (_, MotionAtom::SlideContent) => "SlideContent",
        _ => motion_atom_label(atom),
    };
    if ty == "MotionTrack" {
        format!("MotionTrack::{name}")
    } else {
        format!("{ty}::{name}")
    }
}

fn atom_tracks(path: &str, atom: MotionAtom) -> Vec<(&'static str, &'static str)> {
    match (path, atom) {
        ("/widgets/modal", MotionAtom::FromTop) => {
            vec![("surface.translate_y", "surface.translate_y from top")]
        }
        ("/widgets/modal", MotionAtom::FromBottom) => {
            vec![("surface.translate_y", "surface.translate_y from bottom")]
        }
        ("/widgets/modal", MotionAtom::FromLeft) => {
            vec![("surface.translate_x", "surface.translate_x from left")]
        }
        ("/widgets/modal", MotionAtom::FromRight) => {
            vec![("surface.translate_x", "surface.translate_x from right")]
        }
        ("/widgets/drawer", MotionAtom::FromSide) => vec![("panel.translate_x", "panel.from side")],
        ("/widgets/drawer", MotionAtom::FromLeft) => vec![("panel.translate_x", "panel.from left")],
        ("/widgets/drawer", MotionAtom::FromRight) => {
            vec![("panel.translate_x", "panel.from right")]
        }
        ("/widgets/drawer", MotionAtom::FromTop) => vec![("panel.translate_y", "panel.from top")],
        ("/widgets/drawer", MotionAtom::FromBottom) => {
            vec![("panel.translate_y", "panel.from bottom")]
        }
        ("/widgets/toast", MotionAtom::FromTop) => {
            vec![("surface.translate_y", "surface.from top")]
        }
        ("/widgets/toast", MotionAtom::FromBottom) => {
            vec![("surface.translate_y", "surface.from bottom")]
        }
        (_, MotionAtom::Fade) => vec![
            ("surface.opacity", "surface.opacity"),
            ("backdrop.opacity", "backdrop.opacity where present"),
        ],
        (_, MotionAtom::Scale | MotionAtom::OriginScale | MotionAtom::Pop) => {
            vec![("surface.scale", "surface.scale")]
        }
        (_, MotionAtom::Collapse) => vec![("panel.height", "panel.height")],
        (_, MotionAtom::Chevron) => vec![("indicator.rotation", "indicator.rotation")],
        (_, MotionAtom::Indicator) => vec![
            ("indicator.translate_x", "indicator.translate_x"),
            ("indicator.width", "indicator.width"),
        ],
        (_, MotionAtom::FadeContent) => vec![("content.opacity", "content.opacity")],
        (_, MotionAtom::SlideContent) => vec![("content.translate_x", "content.translate_x")],
        (_, MotionAtom::HoverScale) => vec![("root.scale:hover", "root.scale on hover")],
        (_, MotionAtom::PressScale) => vec![("root.scale:press", "root.scale on press")],
        (_, MotionAtom::Ripple) => vec![("ripple.spawn", "ripple.spawn")],
        (_, MotionAtom::Width) => vec![("rail.width", "rail.width")],
        (_, MotionAtom::FromLeft) => vec![("root.translate_x", "root.translate_x from left")],
        (_, MotionAtom::FromRight) => vec![("root.translate_x", "root.translate_x from right")],
        (_, MotionAtom::FromTop) => vec![("root.translate_y", "root.translate_y from top")],
        (_, MotionAtom::FromBottom) => vec![("root.translate_y", "root.translate_y from bottom")],
        (_, MotionAtom::FromSide) => vec![("root.translate_x", "root.translate_x from side")],
    }
}
