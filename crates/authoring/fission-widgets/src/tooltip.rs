use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, scale_in, slide_y_in, slot_id, SLOT_SURFACE,
};
use fission_core::motion::{MotionTrack, Presence};
use fission_core::ui::{Container, Text, Widget};
use fission_core::{WidgetId, WidgetIdExt};
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Tooltip`].
///
/// Tooltips render statically unless [`Tooltip::motion`] is set. Presets lower
/// to presence tracks for the stable `surface` slot.
///
/// ```rust,ignore
/// let motion = Some(TooltipMotion::FadeAndSlide);
/// ```
pub enum TooltipMotion {
    /// Curated default tooltip motion.
    Default,
    /// Fade the tooltip surface.
    Fade,
    /// Scale the tooltip surface.
    Scale,
    /// Compound fade plus placement-aware slide preset.
    FadeAndSlide,
    /// Ordered composition of tooltip motion atoms.
    Composition(Vec<TooltipMotion>),
    /// Caller-provided surface tracks.
    Custom {
        /// Surface enter tracks.
        surface_enter: Vec<MotionTrack>,
        /// Surface exit tracks.
        surface_exit: Vec<MotionTrack>,
        /// Whether the tooltip remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl TooltipMotion {
    /// Flattens and normalizes an ordered tooltip-motion composition.
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

    fn plan(&self) -> TooltipMotionPlan {
        let mut plan = TooltipMotionPlan::default();
        self.append_plan(&mut plan);
        plan.normalize()
    }

    fn append_plan(&self, plan: &mut TooltipMotionPlan) {
        match self {
            Self::Default | Self::FadeAndSlide => {
                Self::Fade.append_plan(plan);
                push_enter_with_exit(&mut plan.enter, &mut plan.exit, slide_y_in(-6.0, 120));
            }
            Self::Fade => push_enter_with_exit(&mut plan.enter, &mut plan.exit, fade_in(100)),
            Self::Scale => {
                push_enter_with_exit(&mut plan.enter, &mut plan.exit, scale_in(0.96, 120));
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

impl Add for TooltipMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct TooltipMotionPlan {
    enter: Vec<MotionTrack>,
    exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl TooltipMotionPlan {
    fn normalize(mut self) -> Self {
        if self.exit.is_empty() {
            self.exit = exit_for(&self.enter);
        }
        self.enter = dedupe(self.enter);
        self.exit = dedupe(self.exit);
        self
    }
}

/// A hover-activated text tooltip displayed near a trigger widget.
///
/// The tooltip appears when the trigger widget is hovered (detected via
/// `view.runtime().interaction.is_hovered`) or when `is_visible` is explicitly
/// set to `true`. The tooltip card is styled using `TooltipTheme` and rendered
/// in the flyout portal layer.
///
/// # Fields
///
/// * `id` - Stable widget identity.
/// * `child` - The trigger widget that the tooltip is attached to.
/// * `text` - The tooltip message text (max width 220px).
/// * `is_visible` - Force the tooltip visible regardless of hover state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tooltip {
    pub id: WidgetId,
    pub child: Widget,
    pub text: String,
    pub is_visible: bool,
    /// Optional explicit tooltip motion. `None` emits no tooltip-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<TooltipMotion>,
}

impl From<Tooltip> for Widget {
    fn from(component: Tooltip) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let theme = &view.env().theme.components.tooltip;

        let trigger_id = fission_ir::WidgetId::derived(this.id.as_u128(), &[]);
        let is_hovered = view.runtime().interaction.is_hovered(trigger_id);
        let show_tooltip = this.is_visible || is_hovered;

        let trigger = Container::new(this.child.clone()).id(trigger_id);

        if show_tooltip || this.motion.is_some() {
            let style = &theme.style;
            let mut tooltip_card: Widget = Container::new(
                Text::new(this.text.clone())
                    .size(style.font_size.unwrap_or(theme.font_size))
                    .color(style.text_color.unwrap_or(theme.text_color))
                    .max_width(style.max_width.unwrap_or(theme.max_width)),
            )
            .bg_fill(
                style
                    .background
                    .clone()
                    .unwrap_or(fission_core::op::Fill::Solid(theme.bg_color)),
            )
            .padding(style.padding_box(theme.padding_x, theme.padding_y))
            .border_radius(style.radius.unwrap_or(theme.radius))
            .shadows(style.outer_shadows())
            .into();
            if let Some(motion) = &this.motion {
                let plan = motion.plan();
                tooltip_card = Presence {
                    id: slot_id(this.id, SLOT_SURFACE),
                    visible: show_tooltip,
                    enter: plan.enter,
                    exit: plan.exit,
                    keep_rendered: plan.keep_rendered,
                    child: tooltip_card,
                    ..Default::default()
                }
                .into();
            }

            let flyout_node = crate::flyout(
                fission_ir::WidgetId::derived(this.id.as_u128(), &[]),
                tooltip_card,
            );
            ctx.register_portal_with_layer(
                fission_core::PortalLayer::Flyout,
                Some(this.id),
                flyout_node,
            );
        }

        trigger
    }
}
