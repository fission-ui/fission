use crate::routes;
use fission::core::action::ShellRouteChanged;
use fission::prelude::*;
use fission::{GlobalState, ReducerContext};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceTab {
    Ergonomic,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionPolicy {
    Full,
    Reduced,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionChoice {
    None,
    Default,
    Fade,
    Scale,
    Directional,
    Composition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionAtom {
    FromTop,
    FromBottom,
    FromLeft,
    FromRight,
    FromSide,
    Fade,
    Scale,
    OriginScale,
    Pop,
    Collapse,
    Chevron,
    Indicator,
    FadeContent,
    SlideContent,
    HoverScale,
    PressScale,
    Ripple,
    Width,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationGalleryState {
    pub current_path: String,
    pub source_tab: SourceTab,
    pub policy: MotionPolicy,
    pub motion: MotionChoice,
    pub playing: bool,
    pub scrub_ms: u16,
    pub composition_atoms: Vec<MotionAtom>,
    pub widget_compositions: BTreeMap<String, Vec<MotionAtom>>,
    pub composer_open: bool,
}

impl Default for AnimationGalleryState {
    fn default() -> Self {
        Self {
            current_path: routes::OVERVIEW.to_string(),
            source_tab: SourceTab::Ergonomic,
            policy: MotionPolicy::Full,
            motion: MotionChoice::Default,
            playing: false,
            scrub_ms: 0,
            composition_atoms: vec![MotionAtom::FromTop, MotionAtom::Fade, MotionAtom::Scale],
            widget_compositions: BTreeMap::new(),
            composer_open: false,
        }
    }
}

impl GlobalState for AnimationGalleryState {}

#[fission_reducer(NavigateTo)]
pub fn navigate_to(state: &mut AnimationGalleryState, path: String) {
    state.current_path = routes::canonical_path(&path);
    state.motion = default_motion_for_path(&state.current_path);
    sync_composition_atoms_for_path(state);
    ensure_current_widget_composition(state);
    sync_policy_for_path(state);
    state.composer_open = false;
    state.scrub_ms = 0;
    state.playing = false;
}

#[fission_reducer(SelectSource)]
pub fn select_source(state: &mut AnimationGalleryState, tab: SourceTab) {
    state.source_tab = tab;
}

#[fission_reducer(SelectPolicy)]
pub fn select_policy(state: &mut AnimationGalleryState, policy: MotionPolicy) {
    state.policy = policy;
}

#[fission_reducer(SelectMotion)]
pub fn select_motion(state: &mut AnimationGalleryState, motion: MotionChoice) {
    state.motion = motion;
    if motion == MotionChoice::Composition {
        ensure_current_widget_composition(state);
    }
    state.scrub_ms = 0;
    state.playing = false;
}

#[fission_reducer(OpenComposer)]
pub fn open_composer(state: &mut AnimationGalleryState) {
    ensure_current_widget_composition(state);
    state.motion = MotionChoice::Composition;
    state.composer_open = true;
    state.scrub_ms = 0;
    state.playing = false;
}

#[fission_reducer(CloseComposer)]
pub fn close_composer(state: &mut AnimationGalleryState) {
    state.composer_open = false;
}

#[fission_reducer(SetCompositionAtoms)]
pub fn set_composition_atoms(state: &mut AnimationGalleryState, atoms: Vec<MotionAtom>) {
    apply_composition_atoms(state, atoms);
}

fn apply_composition_atoms(state: &mut AnimationGalleryState, atoms: Vec<MotionAtom>) {
    if is_widget_path(&state.current_path) {
        state
            .widget_compositions
            .insert(state.current_path.clone(), atoms);
    } else {
        state.composition_atoms = atoms;
    }
    state.motion = MotionChoice::Composition;
    state.scrub_ms = 0;
    state.playing = false;
}

#[fission_reducer(AddCompositionAtom)]
pub fn add_composition_atom(state: &mut AnimationGalleryState, atom: MotionAtom) {
    let mut atoms = current_composition_atoms(state).to_vec();
    atoms.push(atom);
    apply_composition_atoms(state, atoms);
}

#[fission_reducer(RemoveLastCompositionAtom)]
pub fn remove_last_composition_atom(state: &mut AnimationGalleryState) {
    let mut atoms = current_composition_atoms(state).to_vec();
    atoms.pop();
    apply_composition_atoms(state, atoms);
}

#[fission_reducer(ClearCompositionAtoms)]
pub fn clear_composition_atoms(state: &mut AnimationGalleryState) {
    apply_composition_atoms(state, Vec::new());
}

#[fission_reducer(TogglePlay)]
pub fn toggle_play(state: &mut AnimationGalleryState) {
    state.playing = !state.playing;
    if state.playing && state.scrub_ms == 0 {
        state.scrub_ms = 180;
    }
}

#[fission_reducer(ResetTimeline)]
pub fn reset_timeline(state: &mut AnimationGalleryState) {
    state.playing = false;
    state.scrub_ms = 0;
}

#[fission_reducer(ScrubTimeline)]
pub fn scrub_timeline(state: &mut AnimationGalleryState, value: u16) {
    state.scrub_ms = value.min(300);
    state.playing = false;
}

pub fn on_shell_route_changed(
    state: &mut AnimationGalleryState,
    action: ShellRouteChanged,
    _ctx: &mut ReducerContext<AnimationGalleryState>,
) {
    state.current_path = routes::canonical_path(&action.location.pathname);
    state.motion = default_motion_for_path(&state.current_path);
    sync_composition_atoms_for_path(state);
    ensure_current_widget_composition(state);
    sync_policy_for_path(state);
    state.composer_open = false;
    state.scrub_ms = 0;
    state.playing = false;
}

pub fn default_motion_for_path(path: &str) -> MotionChoice {
    if path.starts_with("/composition/") {
        MotionChoice::Composition
    } else {
        MotionChoice::Default
    }
}

fn sync_composition_atoms_for_path(state: &mut AnimationGalleryState) {
    state.composition_atoms = match state.current_path.as_str() {
        "/composition/conflict" | "/composition/last-wins" => {
            vec![MotionAtom::FromTop, MotionAtom::FromBottom]
        }
        "/composition/additive" => vec![MotionAtom::FromTop, MotionAtom::Fade, MotionAtom::Scale],
        path if path.starts_with("/composition/") && state.composition_atoms.is_empty() => {
            vec![MotionAtom::FromTop, MotionAtom::Fade, MotionAtom::Scale]
        }
        _ => state.composition_atoms.clone(),
    };
}

fn sync_policy_for_path(state: &mut AnimationGalleryState) {
    state.policy = match state.current_path.as_str() {
        "/policy/full" => MotionPolicy::Full,
        "/policy/reduced" => MotionPolicy::Reduced,
        "/policy/disabled" => MotionPolicy::Disabled,
        _ => state.policy,
    };
}

pub fn current_composition_atoms(state: &AnimationGalleryState) -> &[MotionAtom] {
    if is_widget_path(&state.current_path) {
        state
            .widget_compositions
            .get(&state.current_path)
            .map(Vec::as_slice)
            .unwrap_or(&state.composition_atoms)
    } else {
        &state.composition_atoms
    }
}

pub fn is_widget_path(path: &str) -> bool {
    path.starts_with("/widgets/")
}

fn ensure_current_widget_composition(state: &mut AnimationGalleryState) {
    if is_widget_path(&state.current_path)
        && !state.widget_compositions.contains_key(&state.current_path)
    {
        state.widget_compositions.insert(
            state.current_path.clone(),
            default_composition_atoms_for_path(&state.current_path),
        );
    }
}

pub fn default_composition_atoms_for_path(path: &str) -> Vec<MotionAtom> {
    match path {
        "/widgets/modal" => vec![MotionAtom::FromTop, MotionAtom::Fade, MotionAtom::Scale],
        "/widgets/drawer" => vec![MotionAtom::FromSide, MotionAtom::Fade],
        "/widgets/popover" => vec![MotionAtom::Fade, MotionAtom::Scale],
        "/widgets/tooltip" => vec![MotionAtom::Fade, MotionAtom::Scale],
        "/widgets/toast" => vec![MotionAtom::FromTop, MotionAtom::Fade],
        "/widgets/accordion" => vec![MotionAtom::Collapse, MotionAtom::Fade, MotionAtom::Chevron],
        "/widgets/tabs" => vec![MotionAtom::Indicator, MotionAtom::FadeContent],
        "/widgets/button" => vec![
            MotionAtom::HoverScale,
            MotionAtom::PressScale,
            MotionAtom::Ripple,
        ],
        "/widgets/sidebar" => vec![MotionAtom::Width, MotionAtom::Fade],
        "/widgets/carousel" => vec![MotionAtom::FromRight, MotionAtom::Fade],
        "/widgets/checkbox" | "/widgets/switch" => vec![MotionAtom::Scale, MotionAtom::Fade],
        _ => vec![MotionAtom::FromTop, MotionAtom::Fade, MotionAtom::Scale],
    }
}

pub fn available_composition_atoms_for_path(path: &str) -> &'static [MotionAtom] {
    match path {
        "/widgets/modal" => &[
            MotionAtom::FromTop,
            MotionAtom::FromBottom,
            MotionAtom::FromLeft,
            MotionAtom::FromRight,
            MotionAtom::Fade,
            MotionAtom::Scale,
        ],
        "/widgets/drawer" => &[
            MotionAtom::FromSide,
            MotionAtom::FromLeft,
            MotionAtom::FromRight,
            MotionAtom::FromTop,
            MotionAtom::FromBottom,
            MotionAtom::Fade,
        ],
        "/widgets/popover" => &[MotionAtom::Fade, MotionAtom::Scale, MotionAtom::OriginScale],
        "/widgets/tooltip" => &[MotionAtom::Fade, MotionAtom::Scale, MotionAtom::FromTop],
        "/widgets/toast" => &[
            MotionAtom::FromTop,
            MotionAtom::FromBottom,
            MotionAtom::Fade,
            MotionAtom::Pop,
        ],
        "/widgets/accordion" => &[MotionAtom::Collapse, MotionAtom::Fade, MotionAtom::Chevron],
        "/widgets/tabs" => &[
            MotionAtom::Indicator,
            MotionAtom::FadeContent,
            MotionAtom::SlideContent,
        ],
        "/widgets/button" => &[
            MotionAtom::HoverScale,
            MotionAtom::PressScale,
            MotionAtom::Ripple,
        ],
        "/widgets/sidebar" => &[MotionAtom::Width, MotionAtom::Fade, MotionAtom::FromLeft],
        "/widgets/carousel" => &[
            MotionAtom::FromLeft,
            MotionAtom::FromRight,
            MotionAtom::Fade,
        ],
        "/widgets/checkbox" | "/widgets/switch" => &[MotionAtom::Scale, MotionAtom::Fade],
        _ => &[
            MotionAtom::FromTop,
            MotionAtom::FromBottom,
            MotionAtom::FromLeft,
            MotionAtom::FromRight,
            MotionAtom::Fade,
            MotionAtom::Scale,
        ],
    }
}

pub fn composition_type_name_for_path(path: &str) -> &'static str {
    match path {
        "/widgets/modal" => "ModalMotion",
        "/widgets/drawer" => "DrawerMotion",
        "/widgets/popover" => "PopoverMotion",
        "/widgets/tooltip" => "TooltipMotion",
        "/widgets/toast" => "ToastMotion",
        "/widgets/accordion" => "AccordionMotion",
        "/widgets/tabs" => "TabsMotion",
        "/widgets/button" => "ButtonMotion",
        "/widgets/checkbox" | "/widgets/switch" | "/widgets/sidebar" | "/widgets/carousel" => {
            "MotionTrack"
        }
        _ => "ModalMotion",
    }
}

pub fn motion_label(choice: MotionChoice) -> &'static str {
    match choice {
        MotionChoice::None => "None",
        MotionChoice::Default => "Default",
        MotionChoice::Fade => "Fade",
        MotionChoice::Scale => "Scale",
        MotionChoice::Directional => "FromTop/Side",
        MotionChoice::Composition => "Composition",
    }
}

pub fn motion_atom_label(atom: MotionAtom) -> &'static str {
    match atom {
        MotionAtom::FromTop => "FromTop",
        MotionAtom::FromBottom => "FromBottom",
        MotionAtom::FromLeft => "FromLeft",
        MotionAtom::FromRight => "FromRight",
        MotionAtom::FromSide => "FromSide",
        MotionAtom::Fade => "Fade",
        MotionAtom::Scale => "Scale",
        MotionAtom::OriginScale => "OriginScale",
        MotionAtom::Pop => "Pop",
        MotionAtom::Collapse => "Collapse",
        MotionAtom::Chevron => "Chevron",
        MotionAtom::Indicator => "Indicator",
        MotionAtom::FadeContent => "FadeContent",
        MotionAtom::SlideContent => "SlideContent",
        MotionAtom::HoverScale => "HoverScale",
        MotionAtom::PressScale => "PressScale",
        MotionAtom::Ripple => "Ripple",
        MotionAtom::Width => "Width",
    }
}

pub fn policy_label(policy: MotionPolicy) -> &'static str {
    match policy {
        MotionPolicy::Full => "Full",
        MotionPolicy::Reduced => "Reduced",
        MotionPolicy::Disabled => "Disabled",
    }
}
