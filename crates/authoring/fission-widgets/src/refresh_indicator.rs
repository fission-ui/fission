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
    /// No pull or refresh is active and the indicator is hidden.
    Inactive,
    /// The user is pulling but has not crossed the trigger distance.
    Drag,
    /// The pull crossed the trigger distance and will refresh if released.
    Armed,
    /// Refresh work is running and progress is indeterminate.
    Refreshing,
    /// Refresh work completed and the indicator may settle away.
    Done,
}

/// Adds a pull-to-refresh affordance above scrollable content.
///
/// The widget is intentionally stateless. Store the current status and pulled
/// distance in application state, update them from drag reducer input, and
/// provide an `on_refresh` action that starts the refresh work.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefreshIndicator {
    /// Stable identity for gesture handling and the derived progress indicator.
    pub id: WidgetId,
    /// Scrollable content displaced by the pull interaction.
    pub child: Widget,
    /// Current controlled interaction lifecycle.
    pub status: RefreshIndicatorStatus,
    /// Current controlled pull distance in logical pixels.
    pub pulled_extent: f32,
    /// Pull distance required to enter [`RefreshIndicatorStatus::Armed`].
    pub trigger_distance: f32,
    /// Resting content displacement while refresh work is active.
    pub displacement: f32,
    /// Additional distance between the viewport edge and indicator.
    pub edge_offset: f32,
    /// Optional progress foreground color.
    pub color: Option<Color>,
    /// Optional indicator surface color.
    pub background_color: Option<Color>,
    /// Optional circular progress track color.
    pub track_color: Option<Color>,
    /// Circular progress stroke width in logical pixels.
    pub stroke_width: f32,
    /// Logical square size of the progress indicator.
    pub indicator_size: f32,
    /// Action dispatched when a pull gesture begins.
    pub on_pull_start: Option<ActionEnvelope>,
    /// Action dispatched with contextual drag input while pulling.
    pub on_pull_update: Option<ActionEnvelope>,
    /// Action dispatched when a pull is cancelled before refresh.
    pub on_pull_cancel: Option<ActionEnvelope>,
    /// Action dispatched when an armed pull is released.
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
    /// Wraps `child` with default pull-to-refresh geometry and colors.
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            ..Default::default()
        }
    }

    /// Sets the application-owned interaction status.
    pub fn status(mut self, status: RefreshIndicatorStatus) -> Self {
        self.status = status;
        self
    }

    /// Sets the non-negative current pull distance in logical pixels.
    pub fn pulled_extent(mut self, pulled_extent: f32) -> Self {
        self.pulled_extent = pulled_extent.max(0.0);
        self
    }

    /// Sets the positive distance at which a pull becomes armed.
    pub fn trigger_distance(mut self, trigger_distance: f32) -> Self {
        self.trigger_distance = trigger_distance.max(1.0);
        self
    }

    /// Sets the non-negative content displacement maintained during refresh.
    pub fn displacement(mut self, displacement: f32) -> Self {
        self.displacement = displacement.max(0.0);
        self
    }

    /// Sets the non-negative offset between viewport edge and indicator.
    pub fn edge_offset(mut self, edge_offset: f32) -> Self {
        self.edge_offset = edge_offset.max(0.0);
        self
    }

    /// Overrides the progress foreground color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Overrides the indicator surface color.
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Overrides the circular progress track color.
    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = Some(color);
        self
    }

    /// Sets progress stroke width, clamped to at least one logical pixel.
    pub fn stroke_width(mut self, stroke_width: f32) -> Self {
        self.stroke_width = stroke_width.max(1.0);
        self
    }

    /// Sets indicator size, clamped to at least one logical pixel.
    pub fn indicator_size(mut self, indicator_size: f32) -> Self {
        self.indicator_size = indicator_size.max(1.0);
        self
    }

    /// Sets the action dispatched when pulling begins.
    pub fn on_pull_start(mut self, action: ActionEnvelope) -> Self {
        self.on_pull_start = Some(action);
        self
    }

    /// Sets the action receiving contextual pull updates.
    pub fn on_pull_update(mut self, action: ActionEnvelope) -> Self {
        self.on_pull_update = Some(action);
        self
    }

    /// Sets the action dispatched when pulling is cancelled.
    pub fn on_pull_cancel(mut self, action: ActionEnvelope) -> Self {
        self.on_pull_cancel = Some(action);
        self
    }

    /// Sets the action dispatched when an armed pull requests refresh.
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
                motion: Some(crate::CircularProgressMotion::Default),
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
