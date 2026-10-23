use crate::model::InboxState;
use fission::core::ui::{Clip, Transform};
use fission::motion::MotionTransition;
use fission::prelude::*;

const PREVIEW_WIDTH: f32 = 160.0;
const PREVIEW_HEIGHT: f32 = 96.0;
const PREVIEW_ICON_SIZE: f32 = 18.0;
const PREVIEW_ICON_ROTATION_RADIANS: f32 = 0.18;
const PREVIEW_HOVER_OPACITY: f32 = 0.94;
const PREVIEW_PRESSED_SCALE: f32 = 0.98;
const PREVIEW_TRANSITION_MS: u64 = 120;

fission_component! {
    pub SettingsThemePreview<InboxState> {
        on_press: ActionEnvelope,
        theme_name: &'static str,
        background: Color,
        accent: Color,
        is_active: bool,
    } |preview, _state, cx| {
        let active_label = cx.tr("settings.theme.active");
        let tokens = &cx.view.env().theme.tokens;
        let (sin, cos) = PREVIEW_ICON_ROTATION_RADIANS.sin_cos();
        let rotation = [
            cos, -sin, 0.0, 0.0, sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let identifier = format!("inbox.settings.theme.{}", preview.theme_name.to_lowercase());
    } =>
        Pressable(
            Clip(
            path: Some(format!("inset(0px round {}px)", tokens.radii.large)),
            child: Container(
                ZStack(
                children: [
                    Container(Spacer())
                        .size(PREVIEW_WIDTH, PREVIEW_HEIGHT)
                        .bg(preview.background),
                    Positioned(
                        top: Some(tokens.spacing.s),
                        right: Some(tokens.spacing.s),
                        child: if preview.is_active {
                            Badge(text: active_label)
                        },
                    ),
                    Positioned(
                        left: Some(tokens.spacing.s),
                        bottom: Some(tokens.spacing.s),
                        child: Transform(
                                Icon::svg(material::action::check_circle::regular())
                                    .size(PREVIEW_ICON_SIZE)
                                    .color(preview.accent),
                                rotation,
                        ),
                    ),
                ],
                ),
            )
            .size(PREVIEW_WIDTH, PREVIEW_HEIGHT)
            ),
        )
        .id(WidgetId::explicit(&identifier))
        .semantics_identifier(identifier)
        .label(format!("Use {} theme", preview.theme_name))
        .role(PressableRole::Button)
        .on_press(preview.on_press.clone())
        .hover(PressableStyle {
            opacity: Some(PREVIEW_HOVER_OPACITY),
            ..Default::default()
        })
        .pressed(PressableStyle {
            scale: Some(PREVIEW_PRESSED_SCALE),
            ..Default::default()
        })
        .transition(MotionTransition::ease_out(PREVIEW_TRANSITION_MS))
}
