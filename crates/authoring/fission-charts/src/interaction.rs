use fission_core::{Action, ActionId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartTooltipTrigger {
    None,
    Item,
    Axis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartSelectionMode {
    None,
    Single,
    Multiple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartBrushType {
    Rect,
    Horizontal,
    Vertical,
    Polygon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartLegendSelectionMode {
    Static,
    Toggle,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartEmphasisFocus {
    None,
    Series,
    Data,
    Adjacent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartToolAction {
    Restore,
    SaveImage,
    DataZoom,
    Brush,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartEmphasis {
    pub enabled: bool,
    pub focus: ChartEmphasisFocus,
    pub scale: f32,
}

impl Default for ChartEmphasis {
    fn default() -> Self {
        Self {
            enabled: false,
            focus: ChartEmphasisFocus::None,
            scale: 1.08,
        }
    }
}

impl ChartEmphasis {
    pub fn series() -> Self {
        Self {
            enabled: true,
            focus: ChartEmphasisFocus::Series,
            ..Self::default()
        }
    }

    pub fn data() -> Self {
        Self {
            enabled: true,
            focus: ChartEmphasisFocus::Data,
            ..Self::default()
        }
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartBrush {
    pub enabled: bool,
    pub brush_type: ChartBrushType,
    pub preview_rect: Option<(f32, f32, f32, f32)>,
}

impl Default for ChartBrush {
    fn default() -> Self {
        Self {
            enabled: false,
            brush_type: ChartBrushType::Rect,
            preview_rect: None,
        }
    }
}

impl ChartBrush {
    pub fn rect() -> Self {
        Self {
            enabled: true,
            brush_type: ChartBrushType::Rect,
            preview_rect: None,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            enabled: true,
            brush_type: ChartBrushType::Horizontal,
            preview_rect: None,
        }
    }

    pub fn vertical() -> Self {
        Self {
            enabled: true,
            brush_type: ChartBrushType::Vertical,
            preview_rect: None,
        }
    }

    pub fn preview_rect(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.preview_rect = Some((x, y, width, height));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartInteraction {
    pub enabled: bool,
    pub tooltip_trigger: ChartTooltipTrigger,
    pub selection_mode: ChartSelectionMode,
    pub legend_selection: ChartLegendSelectionMode,
    pub brush: Option<ChartBrush>,
    pub emphasis: ChartEmphasis,
    pub toolbox_actions: Vec<ChartToolAction>,
    pub emit_events: bool,
    pub keyboard_focus: bool,
}

impl Default for ChartInteraction {
    fn default() -> Self {
        Self {
            enabled: false,
            tooltip_trigger: ChartTooltipTrigger::None,
            selection_mode: ChartSelectionMode::None,
            legend_selection: ChartLegendSelectionMode::Static,
            brush: None,
            emphasis: ChartEmphasis::default(),
            toolbox_actions: Vec::new(),
            emit_events: false,
            keyboard_focus: false,
        }
    }
}

impl ChartInteraction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tooltips(trigger: ChartTooltipTrigger) -> Self {
        Self {
            enabled: trigger != ChartTooltipTrigger::None,
            tooltip_trigger: trigger,
            ..Self::default()
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn tooltip_trigger(mut self, trigger: ChartTooltipTrigger) -> Self {
        self.tooltip_trigger = trigger;
        self.enabled |= trigger != ChartTooltipTrigger::None;
        self
    }

    pub fn selection_mode(mut self, mode: ChartSelectionMode) -> Self {
        self.selection_mode = mode;
        self.enabled |= mode != ChartSelectionMode::None;
        self
    }

    pub fn brush(mut self, brush: ChartBrush) -> Self {
        self.brush = Some(brush);
        self.enabled = true;
        self
    }

    pub fn legend_selection(mut self, mode: ChartLegendSelectionMode) -> Self {
        self.legend_selection = mode;
        self.enabled |= mode != ChartLegendSelectionMode::Static;
        self
    }

    pub fn emphasis(mut self, emphasis: ChartEmphasis) -> Self {
        self.enabled |= emphasis.enabled;
        self.emphasis = emphasis;
        self
    }

    pub fn toolbox_actions(mut self, actions: Vec<ChartToolAction>) -> Self {
        self.enabled |= !actions.is_empty();
        self.toolbox_actions = actions;
        self
    }

    pub fn emit_events(mut self, emit: bool) -> Self {
        self.emit_events = emit;
        self.enabled |= emit;
        self
    }

    pub fn keyboard_focus(mut self, focusable: bool) -> Self {
        self.keyboard_focus = focusable;
        self.enabled |= focusable;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartInteractionKind {
    Hover,
    Press,
    Release,
    Scroll,
    Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartHitKind {
    SeriesItem,
    PlotArea,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartHit {
    pub kind: ChartHitKind,
    pub series_index: Option<usize>,
    pub series_name: Option<String>,
    pub data_index: Option<usize>,
    pub value_x: Option<f32>,
    pub value_y: Option<f32>,
}

impl ChartHit {
    pub fn plot_area() -> Self {
        Self {
            kind: ChartHitKind::PlotArea,
            series_index: None,
            series_name: None,
            data_index: None,
            value_x: None,
            value_y: None,
        }
    }

    pub fn series_item(
        series_index: usize,
        series_name: impl Into<String>,
        data_index: usize,
        value_x: Option<f32>,
        value_y: Option<f32>,
    ) -> Self {
        Self {
            kind: ChartHitKind::SeriesItem,
            series_index: Some(series_index),
            series_name: Some(series_name.into()),
            data_index: Some(data_index),
            value_x,
            value_y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartInteractionEvent {
    /// Stable retained identity of the chart that produced this event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<fission_core::WidgetId>,
    /// Legacy title-derived identifier. Prefer [`Self::source_id`].
    pub chart_id: Option<String>,
    pub kind: ChartInteractionKind,
    pub local_x: f32,
    pub local_y: f32,
    pub modifiers: u8,
    pub hit: Option<ChartHit>,
}

impl ChartInteractionEvent {
    /// Versioned schema name used by component-bound chart interaction input.
    pub const EVENT_TYPE: &'static str = "fission.chart.interaction.v1";

    /// Decodes the chart interaction accompanying a component-bound action.
    pub fn from_action_input(
        input: &fission_core::ActionInput,
    ) -> Option<(fission_core::WidgetId, Self)> {
        let (source, mut event): (_, Self) =
            input.decode_component_interaction(Self::EVENT_TYPE)?;
        if event.source_id.is_some_and(|encoded| encoded != source) {
            return None;
        }
        event.source_id = Some(source);
        Some((source, event))
    }
}

impl Action for ChartInteractionEvent {
    fn static_id() -> ActionId {
        ActionId::from_name("fission_charts::ChartInteractionEvent")
    }
}
