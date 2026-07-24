use crate::model::{InboxState, SetTheme};
use fission::core::op::Color;
use fission::core::ui::widgets::{Clip, Spacer, Transform};
use fission::core::ui::{Container, Positioned, Widget, ZStack};
use fission::core::{ActionEnvelope, ActionId};
use fission::icons::material;
use fission::motion::MotionTransition;
use fission::prelude::{Pressable, PressableRole, PressableStyle, WidgetId};
use fission::widgets::{Badge, Icon};

const PREVIEW_WIDTH: f32 = 160.0;
const PREVIEW_HEIGHT: f32 = 96.0;
const PREVIEW_ICON_SIZE: f32 = 18.0;
const PREVIEW_ICON_ROTATION_RADIANS: f32 = 0.18;
const PREVIEW_HOVER_OPACITY: f32 = 0.94;
const PREVIEW_PRESSED_SCALE: f32 = 0.98;
const PREVIEW_TRANSITION_MS: u64 = 120;

pub struct SettingsThemePreview {
    pub action_id: ActionId,
    pub theme_name: &'static str,
    pub background: Color,
    pub accent: Color,
    pub is_active: bool,
}

impl From<SettingsThemePreview> for Widget {
    fn from(preview: SettingsThemePreview) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let active_label = view
            .env()
            .i18n
            .get(&view.env().locale, "settings.theme.active")
            .map(str::to_owned)
            .unwrap_or_else(|| "settings.theme.active".to_string());
        let tokens = &view.env().theme.tokens;
        let (sin, cos) = PREVIEW_ICON_ROTATION_RADIANS.sin_cos();
        let rotation = [
            cos, -sin, 0.0, 0.0, sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        let active_badge = preview.is_active.then(|| {
            Badge {
                text: active_label,
                ..Default::default()
            }
            .into()
        });

        let identifier = format!("inbox.settings.theme.{}", preview.theme_name.to_lowercase());

        Pressable::new(Clip {
            id: None,
            path: Some(format!("inset(0px round {}px)", tokens.radii.large)),
            child: Container::new(ZStack {
                children: vec![
                    Container::new(Spacer::default())
                        .size(PREVIEW_WIDTH, PREVIEW_HEIGHT)
                        .bg(preview.background)
                        .into(),
                    Positioned {
                        top: Some(tokens.spacing.s),
                        right: Some(tokens.spacing.s),
                        child: active_badge,
                        ..Default::default()
                    }
                    .into(),
                    Positioned {
                        left: Some(tokens.spacing.s),
                        bottom: Some(tokens.spacing.s),
                        child: Some(
                            Transform::new(
                                Icon::svg(material::action::check_circle::regular())
                                    .size(PREVIEW_ICON_SIZE)
                                    .color(preview.accent),
                                rotation,
                            )
                            .into(),
                        ),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            })
            .size(PREVIEW_WIDTH, PREVIEW_HEIGHT)
            .into(),
        })
        .id(WidgetId::explicit(&identifier))
        .semantics_identifier(identifier)
        .label(format!("Use {} theme", preview.theme_name))
        .role(PressableRole::Button)
        .on_press(ActionEnvelope {
            id: preview.action_id,
            payload: serde_json::to_vec(&SetTheme(preview.theme_name.to_string())).unwrap(),
        })
        .hover(PressableStyle {
            opacity: Some(PREVIEW_HOVER_OPACITY),
            ..Default::default()
        })
        .pressed(PressableStyle {
            scale: Some(PREVIEW_PRESSED_SCALE),
            ..Default::default()
        })
        .transition(MotionTransition::ease_out(PREVIEW_TRANSITION_MS))
        .into()
    }
}
