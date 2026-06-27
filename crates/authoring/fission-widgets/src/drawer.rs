use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, slide_x_in, slide_y_in, slot_id,
    SLOT_BACKDROP, SLOT_PANEL,
};
use fission_core::motion::{MotionTrack, Presence};
use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{Container, GestureDetector, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};
use std::ops::Add;

/// The edge from which a [`Drawer`] slides out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawerSide {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Drawer`].
///
/// Drawers render without motion unless [`Drawer::motion`] is set. Presets
/// lower to native presence tracks for the stable `backdrop` and `panel` slots.
///
/// ```rust,ignore
/// let motion = Some(DrawerMotion::FromSide + DrawerMotion::Fade);
/// ```
pub enum DrawerMotion {
    /// Curated default: side slide plus fade.
    Default,
    /// Fade the backdrop and panel.
    Fade,
    /// Slide from the drawer's configured [`DrawerSide`].
    FromSide,
    /// Slide the panel from the left edge.
    FromLeft,
    /// Slide the panel from the right edge.
    FromRight,
    /// Slide the panel from the top edge.
    FromTop,
    /// Slide the panel from the bottom edge.
    FromBottom,
    /// Compound fade plus side slide preset.
    FadeAndSlide,
    /// Ordered composition of drawer motion atoms.
    Composition(Vec<DrawerMotion>),
    /// Caller-provided tracks for drawer stable slots.
    Custom {
        /// Optional backdrop tracks.
        backdrop: Option<Vec<MotionTrack>>,
        /// Panel enter tracks.
        panel_enter: Vec<MotionTrack>,
        /// Panel exit tracks.
        panel_exit: Vec<MotionTrack>,
        /// Whether the drawer remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl DrawerMotion {
    /// Flattens and normalizes an ordered drawer-motion composition.
    pub fn compose(items: impl IntoIterator<Item = Self>) -> Self {
        let mut out = Vec::new();
        for item in items {
            item.flatten_into(&mut out);
        }
        match out.len() {
            0 => Self::Composition(Vec::new()),
            1 => out.remove(0),
            _ => Self::Composition(out),
        }
    }

    fn flatten_into(self, out: &mut Vec<Self>) {
        match self {
            Self::Composition(items) => {
                for item in items {
                    item.flatten_into(out);
                }
            }
            item => out.push(item),
        }
    }

    fn plan(&self, side: DrawerSide, width: f32) -> DrawerMotionPlan {
        let mut plan = DrawerMotionPlan::default();
        self.append_plan(side, width, &mut plan);
        plan.normalize()
    }

    fn append_plan(&self, side: DrawerSide, width: f32, plan: &mut DrawerMotionPlan) {
        match self {
            Self::Default | Self::FadeAndSlide => {
                Self::FromSide.append_plan(side, width, plan);
                Self::Fade.append_plan(side, width, plan);
            }
            Self::Fade => {
                push_enter_with_exit(
                    &mut plan.backdrop_enter,
                    &mut plan.backdrop_exit,
                    fade_in(140),
                );
                push_enter_with_exit(&mut plan.panel_enter, &mut plan.panel_exit, fade_in(140));
            }
            Self::FromSide => match side {
                DrawerSide::Left => Self::FromLeft.append_plan(side, width, plan),
                DrawerSide::Right => Self::FromRight.append_plan(side, width, plan),
            },
            Self::FromLeft => {
                push_enter_with_exit(
                    &mut plan.panel_enter,
                    &mut plan.panel_exit,
                    slide_x_in(-width, 190),
                );
            }
            Self::FromRight => {
                push_enter_with_exit(
                    &mut plan.panel_enter,
                    &mut plan.panel_exit,
                    slide_x_in(width, 190),
                );
            }
            Self::FromTop => {
                push_enter_with_exit(
                    &mut plan.panel_enter,
                    &mut plan.panel_exit,
                    slide_y_in(-320.0, 190),
                );
            }
            Self::FromBottom => {
                push_enter_with_exit(
                    &mut plan.panel_enter,
                    &mut plan.panel_exit,
                    slide_y_in(320.0, 190),
                );
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(side, width, plan);
                }
            }
            Self::Custom {
                backdrop,
                panel_enter,
                panel_exit,
                keep_rendered,
            } => {
                if let Some(backdrop) = backdrop {
                    plan.backdrop_enter.extend(backdrop.clone());
                }
                plan.panel_enter.extend(panel_enter.clone());
                plan.panel_exit.extend(panel_exit.clone());
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for DrawerMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct DrawerMotionPlan {
    backdrop_enter: Vec<MotionTrack>,
    backdrop_exit: Vec<MotionTrack>,
    panel_enter: Vec<MotionTrack>,
    panel_exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl DrawerMotionPlan {
    fn normalize(mut self) -> Self {
        if self.backdrop_exit.is_empty() {
            self.backdrop_exit = exit_for(&self.backdrop_enter);
        }
        if self.panel_exit.is_empty() {
            self.panel_exit = exit_for(&self.panel_enter);
        }
        self.backdrop_enter = dedupe(self.backdrop_enter);
        self.backdrop_exit = dedupe(self.backdrop_exit);
        self.panel_enter = dedupe(self.panel_enter);
        self.panel_exit = dedupe(self.panel_exit);
        self
    }
}

/// A slide-out panel from the left or right edge of the screen.
///
/// When `is_open` is `true`, the drawer renders as a portal overlay with a
/// semi-transparent backdrop and a fixed-width panel positioned against the
/// specified `side`. Tapping the backdrop dispatches `on_dismiss`.
///
/// # Fields
///
/// * `side` - `Left` or `Right` edge.
/// * `width` - Panel width in logical pixels (default 300).
/// * `is_open` - Controls visibility.
/// * `on_dismiss` - Action dispatched when the backdrop is tapped.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drawer {
    pub id: WidgetId,
    pub side: DrawerSide,
    pub is_open: bool,
    pub on_dismiss: Option<ActionEnvelope>,
    pub content: Widget,
    pub width: Option<f32>,
    /// Optional explicit drawer motion. `None` emits no drawer-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<DrawerMotion>,
}

impl From<Drawer> for Widget {
    fn from(component: Drawer) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let viewport = view.viewport_size();
        let max_panel_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - 24.0).max(180.0)
        } else {
            this.width.unwrap_or(300.0)
        };
        let width = this.width.unwrap_or(300.0).min(max_panel_width);
        if !this.is_open && this.motion.is_none() {
            return fission_core::ui::widgets::Spacer::default().into();
        }
        let motion_plan = this
            .motion
            .as_ref()
            .map(|motion| motion.plan(this.side, width));

        // Backdrop
        let mut backdrop: Widget = GestureDetector {
            on_tap: this.on_dismiss.clone(),
            child: Container::new(fission_core::ui::widgets::Spacer::default())
                .bg(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 128,
                })
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        }
        .into();
        if let Some(plan) = &motion_plan {
            backdrop = Presence {
                id: slot_id(this.id, SLOT_BACKDROP),
                visible: this.is_open,
                enter: plan.backdrop_enter.clone(),
                exit: plan.backdrop_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: backdrop,
                ..Default::default()
            }
            .into();
        }

        // Drawer Content
        let mut content_node: Widget = Container::new(this.content.clone())
            .bg(tokens.colors.surface)
            .width(width)
            // Height fills parent (Positioned top/bottom 0)
            .shadow(tokens.elevations.level3.unwrap_or(BoxShadow {
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 60,
                },
                blur_radius: 16.0,
                offset: (0.0, 0.0),
            }))
            .padding_all(0.0)
            .into();
        if let Some(plan) = &motion_plan {
            content_node = Presence {
                id: slot_id(this.id, SLOT_PANEL),
                visible: this.is_open,
                enter: plan.panel_enter.clone(),
                exit: plan.panel_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: content_node,
                ..Default::default()
            }
            .into();
        }

        let positioned_content = match this.side {
            DrawerSide::Left => fission_core::ui::Positioned {
                left: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                right: None,
                width: Some(width),
                child: Some(content_node),
                ..Default::default()
            },
            DrawerSide::Right => fission_core::ui::Positioned {
                right: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                left: None,
                width: Some(width),
                child: Some(content_node),
                ..Default::default()
            },
        }
        .into();

        let root = ZStack {
            children: vec![
                fission_core::ui::Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop),
                    ..Default::default()
                }
                .into(),
                positioned_content,
            ],
            id: None,
        }
        .into();

        let overlay_root = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(root),
            ..Default::default()
        }
        .into();
        ctx.register_portal_with_layer(
            fission_core::PortalLayer::Modal,
            Some(this.id),
            overlay_root,
        );

        fission_core::ui::widgets::Spacer::default().into()
    }
}
