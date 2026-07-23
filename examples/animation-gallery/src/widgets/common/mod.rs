mod composer_atom_controls;
mod composer_dialog_body;
mod composer_expression;
mod composer_readout;
mod composer_readout_grid;
mod composition_dialog;
mod composition_lowering;
mod controls_panel;
mod current_values;
mod inspector_group;
mod inspector_panel;
mod policy_control;
mod preview_shell;
mod source_tabs;
mod timeline_control;
mod widget_page;
mod widget_workspace;

pub use composition_dialog::CompositionDialog;
pub use controls_panel::ControlsPanel;
pub use current_values::CurrentValues;
pub use inspector_panel::InspectorPanel;
pub use policy_control::PolicyControl;
pub use preview_shell::PreviewShell;
pub use source_tabs::SourceTabs;
pub use timeline_control::TimelineControl;
pub use widget_page::WidgetPage;

use crate::state::{AnimationGalleryState, MotionChoice, MotionPolicy};
use fission::prelude::*;

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
    pub tint: Color,
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
