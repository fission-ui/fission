use fission_ir::{op::Color as IrColor, AnyRenderObject};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextAlignVertical {
    /// Centres single-line fields and top-aligns multiline fields.
    #[default]
    Auto,
    /// Aligns editable content to the top edge.
    Top,
    /// Aligns editable content to the vertical centre.
    Center,
    /// Aligns editable content to the bottom edge.
    Bottom,
}

impl TextAlignVertical {
    pub(crate) fn resolve(self, multiline: bool) -> Self {
        match self {
            Self::Auto if multiline => Self::Top,
            Self::Auto => Self::Center,
            explicit => explicit,
        }
    }

    pub(crate) fn justify_content(self, multiline: bool) -> fission_ir::op::JustifyContent {
        match self.resolve(multiline) {
            Self::Top => fission_ir::op::JustifyContent::Start,
            Self::Center => fission_ir::op::JustifyContent::Center,
            Self::Bottom => fission_ir::op::JustifyContent::End,
            Self::Auto => unreachable!("automatic text alignment must resolve before lowering"),
        }
    }

    pub(crate) fn align_items(self, multiline: bool) -> fission_ir::op::AlignItems {
        match self.resolve(multiline) {
            Self::Top => fission_ir::op::AlignItems::Start,
            Self::Center => fission_ir::op::AlignItems::Center,
            Self::Bottom => fission_ir::op::AlignItems::End,
            Self::Auto => unreachable!("automatic text alignment must resolve before lowering"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DragStartBehavior {
    #[default]
    Start,
    Down,
}

pub use fission_ir::semantics::TextWrapMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextScrollPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextScrollPhysics {
    #[default]
    Platform,
    Clamped,
    NeverScrollable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextValidationResult {
    pub state: fission_ir::semantics::TextFieldValidationState,
    pub message: Option<String>,
}

impl TextValidationResult {
    pub fn valid() -> Self {
        Self {
            state: fission_ir::semantics::TextFieldValidationState::Valid,
            message: None,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            state: fission_ir::semantics::TextFieldValidationState::Invalid,
            message: Some(message.into()),
        }
    }
}

pub trait TextInputValidator: Send + Sync + std::fmt::Debug {
    fn validate(&self, value: &crate::TextEditingValue) -> TextValidationResult;
}

pub type SharedTextInputValidator = std::sync::Arc<dyn TextInputValidator>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextUndoController {
    pub capacity: usize,
}

impl Default for TextUndoController {
    fn default() -> Self {
        Self { capacity: 100 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellCheckConfiguration {
    pub enabled: bool,
    pub underline_color: Option<IrColor>,
    pub show_suggestions: bool,
}

impl Default for SpellCheckConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            underline_color: Some(IrColor {
                r: 255,
                g: 59,
                b: 48,
                a: 255,
            }),
            show_suggestions: true,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct TextInputRuntimeConfig {
    pub drag_start_behavior: DragStartBehavior,
    pub undo_controller: Option<TextUndoController>,
    pub restoration_id: Option<String>,
    pub spell_check_configuration: Option<SpellCheckConfiguration>,
    pub custom_input_formatters: Vec<crate::SharedTextInputFormatter>,
    pub select_all_on_focus: bool,
    pub scroll_policy: TextScrollPolicy,
    pub scroll_physics: TextScrollPhysics,
    pub form_id: Option<String>,
    pub validator: Option<SharedTextInputValidator>,
}

#[doc(hidden)]
pub fn downcast_text_input_runtime_config(
    any: &AnyRenderObject,
) -> Option<&TextInputRuntimeConfig> {
    any.downcast_ref::<TextInputRuntimeConfig>()
}

pub(crate) fn text_input_scroll_physics_for_node(
    ir: &fission_ir::CoreIR,
    node_id: fission_ir::WidgetId,
) -> Option<TextScrollPhysics> {
    let mut current = Some(node_id);
    while let Some(id) = current {
        if let Some(config) = ir
            .custom_render_objects
            .get(&id)
            .and_then(downcast_text_input_runtime_config)
        {
            return Some(config.scroll_physics);
        }
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSelectionControls {
    #[serde(default = "default_selection_controls_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub show_collapsed_handle: bool,
    pub handle_radius: f32,
    pub handle_fill: IrColor,
    pub handle_stroke: Option<IrColor>,
    pub handle_stroke_width: f32,
}

fn default_selection_controls_enabled() -> bool {
    true
}

impl Default for TextSelectionControls {
    fn default() -> Self {
        Self {
            enabled: true,
            show_collapsed_handle: false,
            handle_radius: 7.0,
            handle_fill: IrColor {
                r: 0,
                g: 122,
                b: 255,
                a: 255,
            },
            handle_stroke: Some(IrColor {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }),
            handle_stroke_width: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMagnifierConfiguration {
    pub enabled: bool,
    pub diameter: f32,
    pub scale: f32,
    pub border_radius: f32,
    pub border_color: Option<IrColor>,
    pub border_width: f32,
}

impl Default for TextMagnifierConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            diameter: 84.0,
            scale: 1.4,
            border_radius: 18.0,
            border_color: Some(IrColor {
                r: 210,
                g: 214,
                b: 224,
                a: 255,
            }),
            border_width: 1.0,
        }
    }
}
