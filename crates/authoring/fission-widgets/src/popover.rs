use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, scale_in, slot_id, SLOT_SURFACE,
};
use fission_core::motion::{MotionTrack, Presence};
use fission_core::op::Color;
use fission_core::ui::{Container, GestureDetector, Widget};
use fission_core::{ActionEnvelope, WidgetId, WidgetIdExt};
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Popover`].
///
/// Popovers render statically unless [`Popover::motion`] is set. Presets lower
/// to presence tracks for the stable `surface` slot; placement remains normal
/// popover layout behavior.
///
/// ```rust,ignore
/// let motion = Some(PopoverMotion::Fade + PopoverMotion::Scale);
/// ```
pub enum PopoverMotion {
    /// Curated default popover motion.
    Default,
    /// Fade the popover surface.
    Fade,
    /// Scale the popover surface.
    Scale,
    /// Scale from the resolved placement origin where shells support it.
    OriginAwareScale,
    /// Ordered composition of popover motion atoms.
    Composition(Vec<PopoverMotion>),
    /// Caller-provided surface tracks.
    Custom {
        /// Surface enter tracks.
        surface_enter: Vec<MotionTrack>,
        /// Surface exit tracks.
        surface_exit: Vec<MotionTrack>,
        /// Whether the popover remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl PopoverMotion {
    /// Flattens and normalizes an ordered popover-motion composition.
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

    fn plan(&self) -> PopoverMotionPlan {
        let mut plan = PopoverMotionPlan::default();
        self.append_plan(&mut plan);
        plan.normalize()
    }

    fn append_plan(&self, plan: &mut PopoverMotionPlan) {
        match self {
            Self::Default => {
                Self::Fade.append_plan(plan);
                Self::Scale.append_plan(plan);
            }
            Self::Fade => push_enter_with_exit(&mut plan.enter, &mut plan.exit, fade_in(110)),
            Self::Scale | Self::OriginAwareScale => {
                push_enter_with_exit(&mut plan.enter, &mut plan.exit, scale_in(0.96, 130));
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(plan);
                }
            }
            Self::Custom {
                surface_enter,
                surface_exit,
                keep_rendered,
            } => {
                plan.enter.extend(surface_enter.clone());
                plan.exit.extend(surface_exit.clone());
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for PopoverMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct PopoverMotionPlan {
    enter: Vec<MotionTrack>,
    exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl PopoverMotionPlan {
    fn normalize(mut self) -> Self {
        if self.exit.is_empty() {
            self.exit = exit_for(&self.enter);
        }
        self.enter = dedupe(self.enter);
        self.exit = dedupe(self.exit);
        self
    }
}

/// An anchor-relative popup that renders content positioned next to a trigger widget.
///
/// The trigger widget is rendered inline in the normal layout tree. When `is_open`
/// is `true`, the `content` is placed into a flyout portal positioned relative to
/// the trigger's computed rect. An optional transparent backdrop handles dismiss
/// via `on_close`.
///
/// # Fields
///
/// * `id` - Stable widget identity for the portal system.
/// * `is_open` - Controls visibility of the popup content.
/// * `on_toggle` - Action dispatched to toggle the popover.
/// * `on_close` - Action dispatched when the backdrop is tapped (if set, a backdrop is rendered).
/// * `trigger` - The inline widget that the popover is anchored to.
/// * `content` - The popup content rendered in the flyout layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Popover {
    pub id: WidgetId,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
    pub on_close: Option<ActionEnvelope>,

    pub trigger: Widget,
    pub content: Widget,
    /// Optional explicit popover motion. `None` emits no popover-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<PopoverMotion>,
}

impl From<Popover> for Widget {
    fn from(component: Popover) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        // Derive stable anchor ID
        let anchor_id = WidgetId::derived(this.id.as_u128(), &[0]);

        let trigger_wrapper = Container::new(this.trigger.clone())
            .flex_shrink(0.0)
            .id(anchor_id);

        // Wrap trigger in a clickable area if on_toggle provided?
        // Or assume trigger handles clicks.
        // Usually trigger handles clicks.

        if this.is_open || this.motion.is_some() {
            let mut content_node = this.content.clone();
            if let Some(motion) = &this.motion {
                let plan = motion.plan();
                content_node = Presence {
                    id: slot_id(this.id, SLOT_SURFACE),
                    visible: this.is_open,
                    enter: plan.enter,
                    exit: plan.exit,
                    keep_rendered: plan.keep_rendered,
                    child: content_node,
                    ..Default::default()
                }
                .into();
            }
            let flyout_node = crate::flyout(anchor_id, content_node);
            if this.on_close.is_some() {
                let backdrop = GestureDetector {
                    on_tap: this.on_close.clone(),
                    child: Container::new(fission_core::ui::widgets::Spacer::default())
                        .bg(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        })
                        .into(),
                    ..Default::default()
                }
                .into();

                // We need to render [Backdrop, Flyout].
                // Backdrop is ZStack layer 0. Flyout layer 1.
                use fission_core::ui::ZStack;

                let overlay = ZStack {
                    children: vec![
                        fission_core::ui::Positioned {
                            left: Some(0.0),
                            top: Some(0.0),
                            right: Some(0.0),
                            bottom: Some(0.0),
                            child: Some(backdrop),
                            ..Default::default()
                        }
                        .into(),
                        flyout_node,
                    ],
                    ..Default::default()
                }
                .into();

                ctx.register_portal_with_layer(
                    fission_core::PortalLayer::Flyout,
                    Some(this.id),
                    overlay,
                );
            } else {
                ctx.register_portal_with_layer(
                    fission_core::PortalLayer::Flyout,
                    Some(this.id),
                    flyout_node,
                );
            }
        }

        trigger_wrapper
    }
}
