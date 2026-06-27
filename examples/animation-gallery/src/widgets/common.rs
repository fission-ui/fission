use crate::state::{
    available_composition_atoms_for_path, close_composer, composition_type_name_for_path,
    current_composition_atoms, default_composition_atoms_for_path, motion_atom_label, motion_label,
    open_composer, policy_label, reset_timeline, scrub_timeline, select_motion, select_policy,
    select_source, set_composition_atoms, toggle_play, AnimationGalleryState, CloseComposer,
    MotionAtom, MotionChoice, MotionPolicy, OpenComposer, ResetTimeline, ScrubTimeline,
    SelectMotion, SelectPolicy, SelectSource, SetCompositionAtoms, SourceTab, TogglePlay,
};
use crate::style::*;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::widgets::{Modal, ModalAction, ModalMotion};
use fission::{Column, Container, Row, Slider, Text, Widget};

pub const STANDARD_MOTIONS: &[MotionChoice] = &[
    MotionChoice::None,
    MotionChoice::Default,
    MotionChoice::Fade,
    MotionChoice::Composition,
];
pub const DIRECTIONAL_MOTIONS: &[MotionChoice] = &[
    MotionChoice::None,
    MotionChoice::Default,
    MotionChoice::Fade,
    MotionChoice::Directional,
    MotionChoice::Composition,
];
pub const MODAL_MOTIONS: &[MotionChoice] = &[
    MotionChoice::None,
    MotionChoice::Default,
    MotionChoice::Fade,
    MotionChoice::Scale,
    MotionChoice::Directional,
    MotionChoice::Composition,
];
pub const BUTTON_MOTIONS: &[MotionChoice] = &[
    MotionChoice::None,
    MotionChoice::Default,
    MotionChoice::Scale,
    MotionChoice::Composition,
];

pub fn preview_active(state: &AnimationGalleryState) -> bool {
    state.playing || state.scrub_ms > 0
}

pub fn preview_progress(state: &AnimationGalleryState) -> f32 {
    if state.playing {
        1.0
    } else {
        (state.scrub_ms as f32 / 300.0).clamp(0.0, 1.0)
    }
}

pub fn policy_allows_motion(state: &AnimationGalleryState) -> bool {
    state.policy != MotionPolicy::Disabled && state.motion != MotionChoice::None
}

#[derive(Clone, Copy)]
pub struct WidgetSummary {
    pub path: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub glyph: &'static str,
    pub tint: fission::op::Color,
}

#[derive(Clone, Copy)]
pub struct GalleryCase {
    pub title: &'static str,
    pub description: &'static str,
    pub motions: &'static [MotionChoice],
    pub slots: &'static [&'static str],
    pub tracks: &'static [&'static str],
    pub exprs: &'static [&'static str],
    pub ergonomic_source: &'static str,
    pub native_source: &'static str,
    pub declaration_source: &'static str,
    pub test_source: &'static str,
    pub diagnostic: &'static str,
}

pub struct WidgetPage<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub case: GalleryCase,
    pub preview: Widget,
}

impl From<WidgetPage<'_>> for Widget {
    fn from(page: WidgetPage<'_>) -> Self {
        let source = match page.state.source_tab {
            SourceTab::Ergonomic => page.case.ergonomic_source,
            SourceTab::Native => page.case.native_source,
        };

        Column {
            gap: Some(14.0),
            children: vec![
                ui::PageHeader {
                    title: page.case.title,
                    subtitle: page.case.description,
                }
                .into(),
                ControlsPanel {
                    ctx: page.ctx,
                    state: page.state,
                    motions: page.case.motions,
                }
                .into(),
                Row {
                    gap: Some(14.0),
                    children: vec![
                        Container::new(Column {
                            gap: Some(12.0),
                            children: vec![
                                ui::SectionTitle {
                                    title: "Live Preview",
                                }
                                .into(),
                                page.preview,
                                SourceTabs {
                                    ctx: page.ctx,
                                    state: page.state,
                                }
                                .into(),
                                ui::CodeBlock { source }.into(),
                            ],
                            ..Default::default()
                        })
                        .padding_all(14.0)
                        .border(BORDER, 1.0)
                        .border_radius(16.0)
                        .bg(SURFACE)
                        .width(600.0)
                        .into(),
                        InspectorPanel {
                            case: &page.case,
                            state: page.state,
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                CompositionDialog {
                    ctx: page.ctx,
                    state: page.state,
                    case: &page.case,
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct ControlsPanel<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub motions: &'a [MotionChoice],
}

impl From<ControlsPanel<'_>> for Widget {
    fn from(panel: ControlsPanel<'_>) -> Self {
        let motion_buttons = panel
            .motions
            .iter()
            .map(|motion| {
                ui::ChoiceButton {
                    ctx: panel.ctx,
                    label: motion_label(*motion),
                    active: panel.state.motion == *motion,
                    action: SelectMotion(*motion),
                    reducer: select_motion,
                }
                .into()
            })
            .collect();

        Container::new(Column {
            gap: Some(12.0),
            children: vec![
                ui::SectionTitle { title: "Controls" }.into(),
                Row {
                    gap: Some(8.0),
                    children: {
                        let mut children: Vec<Widget> = motion_buttons;
                        children.push(
                            ui::SmallButton {
                                ctx: panel.ctx,
                                label: "Compose...",
                                action: OpenComposer,
                                reducer: open_composer,
                            }
                            .into(),
                        );
                        children
                    },
                    ..Default::default()
                }
                .into(),
                Row {
                    gap: Some(8.0),
                    children: vec![
                        ui::SmallButton {
                            ctx: panel.ctx,
                            label: if panel.state.playing { "Pause" } else { "Play" },
                            action: TogglePlay,
                            reducer: toggle_play,
                        }
                        .into(),
                        ui::SmallButton {
                            ctx: panel.ctx,
                            label: "Reset",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        }
                        .into(),
                        TimelineControl {
                            ctx: panel.ctx,
                            state: panel.state,
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                PolicyControl {
                    ctx: panel.ctx,
                    state: panel.state,
                }
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(14.0)
        .border(BORDER, 1.0)
        .border_radius(16.0)
        .bg(SURFACE)
        .into()
    }
}

pub struct CompositionDialog<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub case: &'a GalleryCase,
}

impl From<CompositionDialog<'_>> for Widget {
    fn from(dialog: CompositionDialog<'_>) -> Self {
        let close = dialog
            .ctx
            .bind(CloseComposer, fission::reduce_with!(close_composer));
        let atoms = current_composition_atoms(dialog.state);

        Modal {
            id: fission::WidgetId::explicit("animation-gallery.composer-dialog"),
            title: format!("Compose {}", dialog.case.title),
            content: ComposerDialogBody {
                ctx: dialog.ctx,
                state: dialog.state,
                case: dialog.case,
                atoms,
            }
            .into(),
            is_open: dialog.state.composer_open,
            on_dismiss: Some(close.clone()),
            actions: vec![ModalAction {
                label: "Done".into(),
                on_press: Some(close),
                is_primary: true,
            }],
            width: Some(720.0),
            motion: Some(ModalMotion::Fade + ModalMotion::Scale),
        }
        .into()
    }
}

struct ComposerDialogBody<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    case: &'a GalleryCase,
    atoms: &'a [MotionAtom],
}

impl From<ComposerDialogBody<'_>> for Widget {
    fn from(body: ComposerDialogBody<'_>) -> Self {
        let path = body.state.current_path.as_str();
        let expression = composition_expression(path, body.atoms);
        let lowered = lowered_composition_tracks(path, body.atoms);
        let mut add_buttons = Vec::new();
        for atom in available_composition_atoms_for_path(path) {
            let mut next = body.atoms.to_vec();
            next.push(*atom);
            add_buttons.push(
                ui::SmallButton {
                    ctx: body.ctx,
                    label: motion_atom_label(*atom),
                    action: SetCompositionAtoms(next),
                    reducer: set_composition_atoms,
                }
                .into(),
            );
        }

        let mut undo = body.atoms.to_vec();
        undo.pop();

        Column {
            gap: Some(12.0),
            children: vec![
                Text::new(format!(
                    "This edits the live {} composition. Close the dialog and press Play to replay it.",
                    body.case.title
                ))
                .size(12.0)
                .color(MUTED)
                .into(),
                Container::new(Column {
                    gap: Some(9.0),
                    children: vec![
                        Text::new("Add atom").size(12.0).color(INK).into(),
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
                                    ctx: body.ctx,
                                    label: "Undo Last",
                                    action: SetCompositionAtoms(undo),
                                    reducer: set_composition_atoms,
                                }
                                .into(),
                                ui::SmallButton {
                                    ctx: body.ctx,
                                    label: "Clear",
                                    action: SetCompositionAtoms(Vec::new()),
                                    reducer: set_composition_atoms,
                                }
                                .into(),
                                ui::SmallButton {
                                    ctx: body.ctx,
                                    label: "Reset Default",
                                    action: SetCompositionAtoms(default_composition_atoms_for_path(
                                        path,
                                    )),
                                    reducer: set_composition_atoms,
                                }
                                .into(),
                                Text::new(format!("{} atoms", body.atoms.len()))
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
                .into(),
                Row {
                    gap: Some(12.0),
                    children: vec![
                        ComposerReadout {
                            title: "Ordered Atoms",
                            body: &atom_sequence(body.atoms),
                        }
                        .into(),
                        ComposerReadout {
                            title: "Lowered Tracks",
                            body: &lowered,
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Container::new(Column {
                    gap: Some(6.0),
                    children: vec![
                        Text::new(format!("{} expression", composition_type_name_for_path(path)))
                            .size(11.0)
                            .color(MUTED)
                            .into(),
                        Text::new(expression)
                            .size(11.0)
                            .line_height(15.0)
                            .color(INK)
                            .max_width(650.0)
                            .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(12.0)
                .border(BORDER, 1.0)
                .border_radius(12.0)
                .bg(SURFACE)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct ComposerReadout<'a> {
    title: &'a str,
    body: &'a str,
}

impl From<ComposerReadout<'_>> for Widget {
    fn from(readout: ComposerReadout<'_>) -> Self {
        Container::new(Column {
            gap: Some(6.0),
            children: vec![
                Text::new(readout.title).size(12.0).color(INK).into(),
                Text::new(readout.body)
                    .size(10.0)
                    .line_height(14.0)
                    .color(MUTED)
                    .max_width(300.0)
                    .max_height(126.0)
                    .into(),
            ],
            ..Default::default()
        })
        .width(318.0)
        .height(170.0)
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(12.0)
        .bg(SURFACE)
        .into()
    }
}

fn atom_sequence(atoms: &[MotionAtom]) -> String {
    if atoms.is_empty() {
        return "empty".into();
    }
    atoms
        .iter()
        .enumerate()
        .map(|(idx, atom)| format!("{}: {}", idx + 1, motion_atom_label(*atom)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn composition_expression(path: &str, atoms: &[MotionAtom]) -> String {
    if atoms.is_empty() {
        return "None".into();
    }
    atoms
        .iter()
        .map(|atom| atom_expression(path, *atom))
        .collect::<Vec<_>>()
        .join(" + ")
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

fn lowered_composition_tracks(path: &str, atoms: &[MotionAtom]) -> String {
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

pub struct SourceTabs<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<SourceTabs<'_>> for Widget {
    fn from(tabs: SourceTabs<'_>) -> Self {
        Row {
            gap: Some(8.0),
            children: vec![
                ui::ChoiceButton {
                    ctx: tabs.ctx,
                    label: "Ergonomic API",
                    active: tabs.state.source_tab == SourceTab::Ergonomic,
                    action: SelectSource(SourceTab::Ergonomic),
                    reducer: select_source,
                }
                .into(),
                ui::ChoiceButton {
                    ctx: tabs.ctx,
                    label: "Verbose Native API",
                    active: tabs.state.source_tab == SourceTab::Native,
                    action: SelectSource(SourceTab::Native),
                    reducer: select_source,
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct PolicyControl<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PolicyControl<'_>> for Widget {
    fn from(control: PolicyControl<'_>) -> Self {
        Row {
            gap: Some(8.0),
            children: vec![
                Text::new("Motion Policy").size(12.0).color(MUTED).into(),
                ui::ChoiceButton {
                    ctx: control.ctx,
                    label: "Full",
                    active: control.state.policy == MotionPolicy::Full,
                    action: SelectPolicy(MotionPolicy::Full),
                    reducer: select_policy,
                }
                .into(),
                ui::ChoiceButton {
                    ctx: control.ctx,
                    label: "Reduced",
                    active: control.state.policy == MotionPolicy::Reduced,
                    action: SelectPolicy(MotionPolicy::Reduced),
                    reducer: select_policy,
                }
                .into(),
                ui::ChoiceButton {
                    ctx: control.ctx,
                    label: "Disabled",
                    active: control.state.policy == MotionPolicy::Disabled,
                    action: SelectPolicy(MotionPolicy::Disabled),
                    reducer: select_policy,
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct TimelineControl<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<TimelineControl<'_>> for Widget {
    fn from(control: TimelineControl<'_>) -> Self {
        Row {
            gap: Some(8.0),
            children: vec![
                Text::new(format!("{}ms", control.state.scrub_ms))
                    .size(12.0)
                    .color(MUTED)
                    .into(),
                Slider {
                    value: control.state.scrub_ms as f32,
                    min: 0.0,
                    max: 300.0,
                    on_change: Some(control.ctx.bind(
                        ScrubTimeline(control.state.scrub_ms),
                        fission::reduce_with!(scrub_timeline),
                    )),
                    ..Default::default()
                }
                .into(),
                ui::SmallButton {
                    ctx: control.ctx,
                    label: "0",
                    action: ScrubTimeline(0),
                    reducer: scrub_timeline,
                }
                .into(),
                ui::SmallButton {
                    ctx: control.ctx,
                    label: "150",
                    action: ScrubTimeline(150),
                    reducer: scrub_timeline,
                }
                .into(),
                ui::SmallButton {
                    ctx: control.ctx,
                    label: "300",
                    action: ScrubTimeline(300),
                    reducer: scrub_timeline,
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct InspectorPanel<'a> {
    pub case: &'a GalleryCase,
    pub state: &'a AnimationGalleryState,
}

impl From<InspectorPanel<'_>> for Widget {
    fn from(panel: InspectorPanel<'_>) -> Self {
        Container::new(Column {
            gap: Some(12.0),
            children: vec![
                ui::SectionTitle { title: "Inspector" }.into(),
                InspectorGroup {
                    title: "Slots",
                    rows: panel.case.slots,
                }
                .into(),
                InspectorGroup {
                    title: "Tracks",
                    rows: panel.case.tracks,
                }
                .into(),
                InspectorGroup {
                    title: "MotionExpr",
                    rows: panel.case.exprs,
                }
                .into(),
                CurrentValues { state: panel.state }.into(),
                ui::PageNote {
                    title: "Diagnostics",
                    body: panel.case.diagnostic,
                }
                .into(),
            ],
            ..Default::default()
        })
        .width(240.0)
        .padding_all(14.0)
        .border(BORDER, 1.0)
        .border_radius(16.0)
        .bg(SURFACE)
        .into()
    }
}

pub struct CurrentValues<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<CurrentValues<'_>> for Widget {
    fn from(values: CurrentValues<'_>) -> Self {
        let t = (values.state.scrub_ms as f32 / 300.0).clamp(0.0, 1.0);
        Column {
            gap: Some(5.0),
            children: vec![
                Text::new("Current Values").size(12.0).color(INK).into(),
                ui::LabelValue {
                    label: "t(progress)",
                    value: &format!("{:.2}", t),
                }
                .into(),
                ui::LabelValue {
                    label: "opacity",
                    value: &format!("{:.2}", 0.35 + 0.65 * t),
                }
                .into(),
                ui::LabelValue {
                    label: "scale",
                    value: &format!("{:.2}", 0.96 + 0.04 * t),
                }
                .into(),
                ui::LabelValue {
                    label: "translateY",
                    value: &format!("{}px", (-24.0 + 24.0 * t).round() as i32),
                }
                .into(),
                ui::LabelValue {
                    label: "policy",
                    value: policy_label(values.state.policy),
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct InspectorGroup<'a> {
    title: &'a str,
    rows: &'a [&'a str],
}

impl From<InspectorGroup<'_>> for Widget {
    fn from(group: InspectorGroup<'_>) -> Self {
        let mut children = vec![Text::new(group.title).size(12.0).color(INK).into()];
        for row in group.rows {
            children.push(ui::LabelRow { label: row }.into());
        }
        Column {
            gap: Some(4.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

pub struct PreviewShell {
    pub child: Widget,
}

impl From<PreviewShell> for Widget {
    fn from(shell: PreviewShell) -> Self {
        Container::new(shell.child)
            .height(190.0)
            .padding_all(18.0)
            .border_radius(18.0)
            .border(BORDER, 1.0)
            .bg(color(241, 245, 250, 255))
            .into()
    }
}

pub const TEST_SOURCE: &str = r#"#[test]
fn modal_from_top_fade_scale_is_deterministic() {
    let mut app = TestHarness::new(AnimationGallery::modal_demo());
    app.press("open_modal");
    app.pump_ms(0);
    app.assert_motion_value("gallery_modal.surface", TranslateY, px(-24.0));
    app.pump_ms(160);
    app.assert_motion_value_between("gallery_modal.surface", Opacity, 0.1, 1.0);
    app.pump_until_rest();
    app.assert_motion_value("gallery_modal.surface", Scale, scalar(1.0));
}"#;

pub const GENERIC_DECLARATION_SOURCE: &str = r#"MotionDeclaration {
    id,
    kind: MotionDeclarationKind::Tracks { tracks },
}"#;

pub const GENERIC_NATIVE_SOURCE: &str = r#"Motion {
    id,
    tracks: vec![MotionTrack {
        property: MotionPropertyId::Opacity,
        phase: MotionPhase::Composite,
        from: MotionStartValue::Explicit(scalar(0.0)),
        to: scalar(1.0),
        transition: MotionTransition::tween(160, MotionEasing::EaseOut),
    }],
    child,
    ..Default::default()
}.into()"#;
