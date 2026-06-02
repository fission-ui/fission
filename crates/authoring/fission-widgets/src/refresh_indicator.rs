use crate::CircularProgress;
use fission_core::op::Color;
use fission_core::ui::{
    Align, Composite, Container, GestureDetector, Positioned, Spacer, Widget, ZStack,
};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};

/// Visual state for a pull-to-refresh interaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefreshIndicatorStatus {
    #[default]
    Inactive,
    Drag,
    Armed,
    Refreshing,
    Done,
}

/// Adds a pull-to-refresh affordance above scrollable content.
///
/// The widget is intentionally stateless. Store the current status and pulled
/// distance in application state, update them from drag reducer input, and
/// provide an `on_refresh` action that starts the refresh work.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefreshIndicator {
    pub id: WidgetId,
    pub child: Widget,
    pub status: RefreshIndicatorStatus,
    pub pulled_extent: f32,
    pub trigger_distance: f32,
    pub displacement: f32,
    pub edge_offset: f32,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub track_color: Option<Color>,
    pub stroke_width: f32,
    pub indicator_size: f32,
    pub on_pull_start: Option<ActionEnvelope>,
    pub on_pull_update: Option<ActionEnvelope>,
    pub on_pull_cancel: Option<ActionEnvelope>,
    pub on_refresh: Option<ActionEnvelope>,
}

impl Default for RefreshIndicator {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("fission.widgets.refresh_indicator"),
            child: Spacer::default().into(),
            status: RefreshIndicatorStatus::Inactive,
            pulled_extent: 0.0,
            trigger_distance: 80.0,
            displacement: 40.0,
            edge_offset: 0.0,
            color: None,
            background_color: None,
            track_color: None,
            stroke_width: 4.0,
            indicator_size: 36.0,
            on_pull_start: None,
            on_pull_update: None,
            on_pull_cancel: None,
            on_refresh: None,
        }
    }
}

impl RefreshIndicator {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            ..Default::default()
        }
    }

    pub fn status(mut self, status: RefreshIndicatorStatus) -> Self {
        self.status = status;
        self
    }

    pub fn pulled_extent(mut self, pulled_extent: f32) -> Self {
        self.pulled_extent = pulled_extent.max(0.0);
        self
    }

    pub fn trigger_distance(mut self, trigger_distance: f32) -> Self {
        self.trigger_distance = trigger_distance.max(1.0);
        self
    }

    pub fn displacement(mut self, displacement: f32) -> Self {
        self.displacement = displacement.max(0.0);
        self
    }

    pub fn edge_offset(mut self, edge_offset: f32) -> Self {
        self.edge_offset = edge_offset.max(0.0);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = Some(color);
        self
    }

    pub fn stroke_width(mut self, stroke_width: f32) -> Self {
        self.stroke_width = stroke_width.max(1.0);
        self
    }

    pub fn indicator_size(mut self, indicator_size: f32) -> Self {
        self.indicator_size = indicator_size.max(1.0);
        self
    }

    pub fn on_pull_start(mut self, action: ActionEnvelope) -> Self {
        self.on_pull_start = Some(action);
        self
    }

    pub fn on_pull_update(mut self, action: ActionEnvelope) -> Self {
        self.on_pull_update = Some(action);
        self
    }

    pub fn on_pull_cancel(mut self, action: ActionEnvelope) -> Self {
        self.on_pull_cancel = Some(action);
        self
    }

    pub fn on_refresh(mut self, action: ActionEnvelope) -> Self {
        self.on_refresh = Some(action);
        self
    }

    fn indicator_progress(&self) -> Option<f32> {
        match self.status {
            RefreshIndicatorStatus::Inactive => Some(0.0),
            RefreshIndicatorStatus::Drag | RefreshIndicatorStatus::Armed => {
                Some((self.pulled_extent / self.trigger_distance.max(1.0)).clamp(0.0, 1.0))
            }
            RefreshIndicatorStatus::Refreshing => None,
            RefreshIndicatorStatus::Done => Some(1.0),
        }
    }

    fn is_indicator_visible(&self) -> bool {
        self.status != RefreshIndicatorStatus::Inactive || self.pulled_extent > 0.0
    }

    fn progress_id(&self) -> WidgetId {
        WidgetId::from_u128(self.id.as_u128() ^ 1)
    }

    fn child_offset(&self) -> f32 {
        match self.status {
            RefreshIndicatorStatus::Inactive => self.pulled_extent.min(self.displacement),
            RefreshIndicatorStatus::Drag | RefreshIndicatorStatus::Armed => {
                self.pulled_extent.min(self.displacement)
            }
            RefreshIndicatorStatus::Refreshing => self.displacement,
            RefreshIndicatorStatus::Done => 0.0,
        }
    }
}

impl From<RefreshIndicator> for Widget {
    fn from(component: RefreshIndicator) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let pull_offset = this.child_offset();
        let indicator_top = this.edge_offset + pull_offset * 0.5;

        let child = if pull_offset > 0.0 {
            Composite::new(this.child.clone())
                .translate_y(pull_offset)
                .into()
        } else {
            this.child.clone()
        };
        let mut children = vec![child];
        if this.is_indicator_visible() {
            let progress: Widget = CircularProgress {
                id: this.progress_id(),
                value: this.indicator_progress(),
                size: this.indicator_size,
                color: Some(this.color.unwrap_or(tokens.colors.primary)),
                track_color: Some(this.track_color.unwrap_or(tokens.colors.border)),
                thickness: this.stroke_width,
                animated: true,
            }
            .into();

            let indicator: Widget = Container::new(progress)
                .size(this.indicator_size + 16.0, this.indicator_size + 16.0)
                .bg(this.background_color.unwrap_or(tokens.colors.surface))
                .border(tokens.colors.border, 1.0)
                .border_radius((this.indicator_size + 16.0) * 0.5)
                .padding_all(8.0)
                .into();

            children.push(
                Positioned {
                    top: Some(indicator_top),
                    left: Some(0.0),
                    right: Some(0.0),
                    height: Some(this.indicator_size + 16.0),
                    child: Some(Align::new(indicator).into()),
                    ..Default::default()
                }
                .into(),
            );
        }

        GestureDetector {
            child: ZStack { id: None, children }.into(),
            on_drag_start: this.on_pull_start.clone(),
            on_drag_update: this.on_pull_update.clone(),
            on_drag_end: if this.status == RefreshIndicatorStatus::Armed {
                this.on_refresh.clone()
            } else {
                this.on_pull_cancel.clone()
            },
            ..Default::default()
        }
        .into()
    }
}
