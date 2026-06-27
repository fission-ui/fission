use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, scale_in, slide_x_in, slide_y_in, slot_id,
    SLOT_BACKDROP, SLOT_SURFACE,
};
use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::motion::{MotionTrack, Presence};
use fission_core::op::Color;
use fission_core::ui::{
    Align, Button, ButtonVariant, Container, GestureDetector, Text, Widget, ZStack,
};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Modal`].
///
/// Modals render without motion unless [`Modal::motion`] is set. Presets lower
/// to native [`Presence`] and [`MotionTrack`] values for the stable `backdrop`
/// and `surface` slots.
///
/// ```rust,ignore
/// let motion = Some(ModalMotion::FromTop + ModalMotion::Fade + ModalMotion::Scale);
/// ```
pub enum ModalMotion {
    /// Curated default modal motion.
    Default,
    /// Fade the backdrop and surface.
    Fade,
    /// Scale the surface from slightly smaller to normal size.
    Scale,
    /// Translate the surface from above.
    FromTop,
    /// Translate the surface from below.
    FromBottom,
    /// Translate the surface from the left.
    FromLeft,
    /// Translate the surface from the right.
    FromRight,
    /// Ordered composition of modal motion atoms.
    Composition(Vec<ModalMotion>),
    /// Caller-provided tracks for modal stable slots.
    Custom {
        /// Optional backdrop tracks.
        backdrop: Option<Vec<MotionTrack>>,
        /// Surface enter tracks.
        surface_enter: Vec<MotionTrack>,
        /// Surface exit tracks.
        surface_exit: Vec<MotionTrack>,
        /// Whether the modal remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl ModalMotion {
    /// Flattens and normalizes an ordered modal-motion composition.
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

    fn plan(&self) -> ModalMotionPlan {
        let mut plan = ModalMotionPlan::default();
        self.append_plan(&mut plan);
        plan.normalize()
    }

    fn append_plan(&self, plan: &mut ModalMotionPlan) {
        match self {
            Self::Default => {
                Self::Fade.append_plan(plan);
                Self::Scale.append_plan(plan);
            }
            Self::Fade => {
                push_enter_with_exit(
                    &mut plan.backdrop_enter,
                    &mut plan.backdrop_exit,
                    fade_in(160),
                );
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    fade_in(160),
                );
            }
            Self::Scale => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    scale_in(0.96, 180),
                );
            }
            Self::FromTop => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_y_in(-28.0, 180),
                );
            }
            Self::FromBottom => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_y_in(28.0, 180),
                );
            }
            Self::FromLeft => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_x_in(-28.0, 180),
                );
            }
            Self::FromRight => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_x_in(28.0, 180),
                );
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(plan);
                }
            }
            Self::Custom {
                backdrop,
                surface_enter,
                surface_exit,
                keep_rendered,
            } => {
                if let Some(backdrop) = backdrop {
                    plan.backdrop_enter.extend(backdrop.clone());
                }
                plan.surface_enter.extend(surface_enter.clone());
                plan.surface_exit.extend(surface_exit.clone());
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for ModalMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct ModalMotionPlan {
    backdrop_enter: Vec<MotionTrack>,
    backdrop_exit: Vec<MotionTrack>,
    surface_enter: Vec<MotionTrack>,
    surface_exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl ModalMotionPlan {
    fn normalize(mut self) -> Self {
        if self.backdrop_exit.is_empty() {
            self.backdrop_exit = exit_for(&self.backdrop_enter);
        }
        if self.surface_exit.is_empty() {
            self.surface_exit = exit_for(&self.surface_enter);
        }
        self.backdrop_enter = dedupe(self.backdrop_enter);
        self.backdrop_exit = dedupe(self.backdrop_exit);
        self.surface_enter = dedupe(self.surface_enter);
        self.surface_exit = dedupe(self.surface_exit);
        self
    }
}

/// A modal dialog with a dimmed backdrop, title bar, content area, and action buttons.
///
/// When `is_open` is `true`, the modal renders as a centered card on a full-screen
/// semi-transparent backdrop. Tapping the backdrop dispatches `on_dismiss`. The modal
/// is rendered into the portal overlay layer (`PortalLayer::Modal`), so it appears
/// above all other content.
///
/// # Fields
///
/// * `id` - Stable widget identity for the portal system.
/// * `title` - Text displayed in the modal header.
/// * `content` - The main body content node.
/// * `is_open` - Controls visibility. When `false`, renders an invisible spacer.
/// * `on_dismiss` - Action dispatched when the backdrop or close button is tapped.
/// * `actions` - Footer buttons (e.g., Cancel, OK).
/// * `width` - Optional fixed width. Falls back to `ModalTheme::max_width` (600px).
///
/// # Example
///
/// ```rust,ignore
/// Modal {
///     id: WidgetId::explicit("confirm"),
///     title: "Delete item?".into(),
///     content: Text::new("This cannot be undone.").into(),
///     is_open: state.show_confirm,
///     on_dismiss: Some(dismiss_action),
///     actions: vec![
///         ModalAction { label: "Cancel".into(), on_press: Some(cancel), is_primary: false },
///         ModalAction { label: "Delete".into(), on_press: Some(delete), is_primary: true },
///     ],
///     width: None,
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Modal {
    pub id: WidgetId,
    pub title: String,
    pub content: Widget,
    pub is_open: bool,
    pub on_dismiss: Option<ActionEnvelope>,
    pub actions: Vec<ModalAction>,
    pub width: Option<f32>,
    /// Optional explicit modal motion. `None` emits no modal-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<ModalMotion>,
}

/// A single action button displayed in the modal footer.
///
/// When `is_primary` is `true`, the button uses `ButtonVariant::Filled` with
/// the primary color. Otherwise it uses `ButtonVariant::Outline`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalAction {
    pub label: String,
    pub on_press: Option<ActionEnvelope>,
    pub is_primary: bool,
}

impl From<Modal> for Widget {
    fn from(component: Modal) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        if !this.is_open && this.motion.is_none() {
            return fission_core::ui::widgets::spacer::Spacer::default().into();
        }
        let motion_plan = this.motion.as_ref().map(ModalMotion::plan);

        let theme = &view.env().theme.components.modal;
        let tokens = &view.env().theme.tokens;
        let container_style = &theme.container_style;
        let viewport = view.viewport_size();
        let horizontal_margin = 24.0;
        let max_dialog_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - horizontal_margin * 2.0).max(280.0)
        } else {
            theme.max_width
        };
        let dialog_width = this.width.unwrap_or(theme.max_width).min(max_dialog_width);

        // Dimmed backdrop
        let backdrop =
            Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                .bg_fill(theme.scrim_style.background.clone().unwrap_or(
                    fission_core::op::Fill::Solid(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 220,
                    }),
                ))
                .flex_grow(1.0)
                .into();

        let mut backdrop_btn: Widget = GestureDetector {
            on_tap: this.on_dismiss.clone(),
            child: backdrop,
            ..Default::default()
        }
        .into();
        if let Some(plan) = &motion_plan {
            backdrop_btn = Presence {
                id: slot_id(this.id, SLOT_BACKDROP),
                visible: this.is_open,
                enter: plan.backdrop_enter.clone(),
                exit: plan.backdrop_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: backdrop_btn,
                ..Default::default()
            }
            .into();
        }

        // Modal Content
        let mut action_buttons = Vec::new();
        for action in &this.actions {
            action_buttons.push(
                Button {
                    variant: if action.is_primary {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::SecondaryGray
                    },
                    child: Some(
                        Text::new(action.label.clone())
                            .color(if action.is_primary {
                                tokens.colors.on_primary
                            } else {
                                tokens.colors.primary
                            })
                            .into(),
                    ),
                    on_press: action.on_press.clone(),
                    ..Default::default()
                }
                .into(),
            );
        }

        let mut modal_card_builder = Container::new(VStack {
            spacing: Some(16.0),
            children: vec![
                // Header
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Text::new(this.title.clone()).size(20.0).into(),
                        fission_core::ui::widgets::spacer::Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(
                                Icon::svg(fission_icons::material::navigation::close::regular())
                                    .size(20.0)
                                    .into(),
                            ),
                            on_press: this.on_dismiss.clone(),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
                // Content
                this.content.clone(),
                // Footer Actions
                HStack {
                    spacing: Some(8.0),
                    children: vec![fission_core::ui::widgets::spacer::Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into()]
                    .into_iter()
                    .chain(action_buttons)
                    .collect(),
                }
                .into(),
            ],
        })
        .bg_fill(
            container_style
                .background
                .clone()
                .unwrap_or(fission_core::op::Fill::Solid(theme.bg_color)),
        )
        .border_radius(container_style.radius.unwrap_or(theme.radius))
        .shadows(container_style.outer_shadows());

        if container_style.shadows.is_empty() {
            if let Some(s) = theme.shadow {
                modal_card_builder = modal_card_builder.shadow(s);
            }
        }

        let mut modal_card: Widget = modal_card_builder
            .width(dialog_width)
            .padding_all(24.0)
            .into();
        if let Some(plan) = &motion_plan {
            modal_card = Presence {
                id: slot_id(this.id, SLOT_SURFACE),
                visible: this.is_open,
                enter: plan.surface_enter.clone(),
                exit: plan.surface_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: modal_card,
                ..Default::default()
            }
            .into();
        }

        let center_layer = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Align::new(modal_card.clone()).into()),
            ..Default::default()
        }
        .into();

        let root = Container::new(ZStack {
            children: vec![
                // Full-screen backdrop button
                fission_core::ui::Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop_btn),
                    ..Default::default()
                }
                .into(),
                // Full-screen container with flex spacers to center the modal card
                center_layer,
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
        .into();

        let positioned_root = fission_core::ui::Positioned {
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
            positioned_root,
        );

        fission_core::ui::widgets::spacer::Spacer::default().into()
    }
}
