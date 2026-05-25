use crate::density::UiDensity;
use crate::state::UiState;
use crate::theme::UiPalette;
use fission::ir::op::Fill;
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

impl Widget<UiState> for ActionButton {
    fn build(&self, _ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let (background, text) = match self.tone {
            ButtonTone::Primary => (palette.accent, palette.accent_text),
            ButtonTone::Neutral => (palette.subtle, palette.text),
            ButtonTone::Success => (palette.success, palette.accent_text),
            ButtonTone::Warning => (palette.warning, palette.accent_text),
        };
        let marker = match self.tone {
            ButtonTone::Primary => ">",
            ButtonTone::Neutral => "-",
            ButtonTone::Success => "+",
            ButtonTone::Warning => "!",
        };
        let label = format!("[{marker} {}]", self.label);
        let density = UiDensity::new(view.state.compact_mode);
        Button {
            on_press: Some(self.action.clone()),
            width: self.width,
            height: Some(density.control_height()),
            padding: Some(density.control_padding()),
            background_fill: Some(Fill::Solid(background)),
            text_color: Some(text),
            child: Some(Box::new(Text::new(label).color(text).into_node())),
            ..Default::default()
        }
        .into_node()
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

impl Widget<UiState> for TogglePill {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let label = if self.enabled {
            format!("[x] {}", self.label)
        } else {
            format!("[ ] {}", self.label)
        };
        let tone = if self.enabled {
            ButtonTone::Primary
        } else {
            ButtonTone::Neutral
        };
        ActionButton::new(label, self.action.clone())
            .tone(tone)
            .build(ctx, view)
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

impl Widget<UiState> for FormTextField {
    fn build(&self, _ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let density = UiDensity::new(view.state.compact_mode);
        TextInput {
            id: Some(NodeId::explicit(self.id)),
            value: self.value.clone(),
            label: Some(self.label.clone().into()),
            placeholder: Some(self.placeholder.clone().into()),
            on_change: Some(self.action.clone()),
            width: Some(self.width),
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
        .into_node()
    }
}
