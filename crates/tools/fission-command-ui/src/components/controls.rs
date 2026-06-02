use crate::density::UiDensity;
use crate::state::UiState;
use crate::theme::UiPalette;
use fission::op::Fill;
use fission::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonTone {
    Primary,
    Neutral,
    Success,
    Warning,
}

#[derive(Clone)]
pub struct ActionButton {
    pub label: String,
    pub action: ActionEnvelope,
    pub tone: ButtonTone,
    pub width: Option<f32>,
}

impl ActionButton {
    pub fn new(label: impl Into<String>, action: ActionEnvelope) -> Self {
        Self {
            label: label.into(),
            action,
            tone: ButtonTone::Neutral,
            width: None,
        }
    }

    pub fn tone(mut self, tone: ButtonTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl From<ActionButton> for Widget {
    fn from(component: ActionButton) -> Self {
        let (_ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let (background, text) = match component.tone {
            ButtonTone::Primary => (palette.accent, palette.accent_text),
            ButtonTone::Neutral => (palette.subtle, palette.text),
            ButtonTone::Success => (palette.success, palette.accent_text),
            ButtonTone::Warning => (palette.warning, palette.accent_text),
        };
        let marker = match component.tone {
            ButtonTone::Primary => ">",
            ButtonTone::Neutral => "-",
            ButtonTone::Success => "+",
            ButtonTone::Warning => "!",
        };
        let label = format!("[{marker} {}]", component.label);
        let density = UiDensity::new(view.state().compact_mode);
        Button {
            on_press: Some(component.action.clone()),
            width: component.width,
            height: Some(density.control_height()),
            padding: Some(density.control_padding()),
            background_fill: Some(Fill::Solid(background)),
            text_color: Some(text),
            child: Some(Text::new(label).color(text).into()),
            ..Default::default()
        }
        .into()
    }
}
#[derive(Clone)]
pub struct TogglePill {
    pub label: String,
    pub enabled: bool,
    pub action: ActionEnvelope,
}

impl TogglePill {
    pub fn new(label: impl Into<String>, enabled: bool, action: ActionEnvelope) -> Self {
        Self {
            label: label.into(),
            enabled,
            action,
        }
    }
}

impl From<TogglePill> for Widget {
    fn from(component: TogglePill) -> Self {
        let (_ctx, _view) = fission::build::current::<UiState>();
        let label = if component.enabled {
            format!("[x] {}", component.label)
        } else {
            format!("[ ] {}", component.label)
        };
        let tone = if component.enabled {
            ButtonTone::Primary
        } else {
            ButtonTone::Neutral
        };
        ActionButton::new(label, component.action.clone())
            .tone(tone)
            .into()
    }
}
#[derive(Clone)]
pub struct FormTextField {
    pub id: &'static str,
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub action: ActionEnvelope,
    pub width: f32,
}

impl FormTextField {
    pub fn new(
        id: &'static str,
        label: impl Into<String>,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        action: ActionEnvelope,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            value: value.into(),
            placeholder: placeholder.into(),
            action,
            width: 32.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl From<FormTextField> for Widget {
    fn from(component: FormTextField) -> Self {
        let (_ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let density = UiDensity::new(view.state().compact_mode);
        TextInput {
            id: Some(WidgetId::explicit(component.id)),
            value: component.value.clone(),
            label: Some(component.label.clone().into()),
            placeholder: Some(component.placeholder.clone().into()),
            on_change: Some(component.action.clone()),
            width: Some(component.width),
            height: Some(density.text_input_height()),
            padding: Some(density.text_input_padding()),
            background_fill: Some(Fill::Solid(palette.surface)),
            border_color: Some(palette.border),
            focus_border_color: Some(palette.accent),
            text_color: Some(palette.text),
            placeholder_color: Some(palette.muted),
            label_color: Some(palette.muted),
            helper_color: Some(palette.muted),
            ..Default::default()
        }
        .into()
    }
}
