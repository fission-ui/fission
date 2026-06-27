use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, scale_in, slide_y_in, slot_id, SLOT_SURFACE,
};
use crate::stack::HStack;
use crate::Icon;
use fission_core::motion::{MotionTrack, Presence};
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonVariant, Container, Text, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Toast`].
///
/// Toasts render statically unless [`Toast::motion`] is set. Presets lower to
/// presence tracks for the stable `surface` slot.
///
/// ```rust,ignore
/// let motion = Some(ToastMotion::Fade + ToastMotion::SlideFromTop);
/// ```
pub enum ToastMotion {
    /// Curated default toast motion.
    Default,
    /// Fade the toast surface.
    Fade,
    /// Slide the toast from above.
    SlideFromTop,
    /// Slide the toast from below.
    SlideFromBottom,
    /// Scale the toast surface from smaller to normal size.
    Pop,
    /// Ordered composition of toast motion atoms.
    Composition(Vec<ToastMotion>),
    /// Caller-provided surface tracks.
    Custom {
        /// Surface enter tracks.
        enter: Vec<MotionTrack>,
        /// Surface exit tracks.
        exit: Vec<MotionTrack>,
        /// Whether the toast remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl ToastMotion {
    /// Flattens and normalizes an ordered toast-motion composition.
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

    fn plan(&self) -> ToastMotionPlan {
        let mut plan = ToastMotionPlan::default();
        self.append_plan(&mut plan);
        plan.normalize()
    }

    fn append_plan(&self, plan: &mut ToastMotionPlan) {
        match self {
            Self::Default => {
                Self::Fade.append_plan(plan);
                Self::SlideFromTop.append_plan(plan);
            }
            Self::Fade => push_enter_with_exit(&mut plan.enter, &mut plan.exit, fade_in(140)),
            Self::SlideFromTop => {
                push_enter_with_exit(&mut plan.enter, &mut plan.exit, slide_y_in(-16.0, 160));
            }
            Self::SlideFromBottom => {
                push_enter_with_exit(&mut plan.enter, &mut plan.exit, slide_y_in(16.0, 160));
            }
            Self::Pop => push_enter_with_exit(&mut plan.enter, &mut plan.exit, scale_in(0.94, 160)),
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(plan);
                }
            }
            Self::Custom {
                enter,
                exit,
                keep_rendered,
            } => {
                plan.enter.extend(enter.clone());
                plan.exit.extend(exit.clone());
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for ToastMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct ToastMotionPlan {
    enter: Vec<MotionTrack>,
    exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl ToastMotionPlan {
    fn normalize(mut self) -> Self {
        if self.exit.is_empty() {
            self.exit = exit_for(&self.enter);
        }
        self.enter = dedupe(self.enter);
        self.exit = dedupe(self.exit);
        self
    }
}

/// The severity level of a [`Toast`] notification, which determines the icon and color.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A notification message with an icon, text, and close button.
///
/// Toasts are typically positioned at the top or bottom of the screen by the
/// application. The icon and color are determined by `kind`: Info (primary),
/// Success (check), Warning (orange triangle), or Error (red circle).
///
/// The toast renders with an elevated shadow and rounded corners. It does not
/// auto-dismiss -- the application must manage its lifecycle and remove it
/// when `on_close` fires or after a timeout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toast {
    pub id: WidgetId,
    pub kind: ToastKind,
    pub message: String,
    pub on_close: Option<ActionEnvelope>,
    /// Optional explicit toast motion. `None` emits no toast-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<ToastMotion>,
}

impl From<Toast> for Widget {
    fn from(component: Toast) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let (icon_path, icon_color) = match this.kind {
            ToastKind::Info => (material::action::info::regular(), tokens.colors.primary),
            ToastKind::Success => (
                material::action::check_circle::regular(),
                tokens.colors.on_background,
            ),
            ToastKind::Warning => (
                material::action::report_problem::regular(),
                Color {
                    r: 255,
                    g: 152,
                    b: 0,
                    a: 255,
                },
            ),
            ToastKind::Error => (material::alert::error::regular(), tokens.colors.error),
        };

        let content: Widget = HStack {
            spacing: Some(12.0),
            children: vec![
                Icon::svg(icon_path).color(icon_color).size(20.0).into(),
                Text::new(this.message.clone())
                    .color(tokens.colors.on_surface)
                    .flex_grow(1.0)
                    .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Icon::svg(material::navigation::close::regular())
                            .size(16.0)
                            .into(),
                    ),
                    on_press: this.on_close.clone(),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into();

        let mut toast: Widget = Container::new(content)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.medium)
            .shadow(
                tokens
                    .elevations
                    .level3
                    .unwrap_or(fission_core::op::BoxShadow {
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 60,
                        },
                        blur_radius: 12.0,
                        offset: (0.0, 6.0),
                    }),
            )
            .padding_all(12.0)
            .into();

        if let Some(motion) = &this.motion {
            let plan = motion.plan();
            toast = Presence {
                id: slot_id(this.id, SLOT_SURFACE),
                visible: true,
                enter: plan.enter,
                exit: plan.exit,
                keep_rendered: plan.keep_rendered,
                child: toast,
                ..Default::default()
            }
            .into();
        }

        toast
    }
}
