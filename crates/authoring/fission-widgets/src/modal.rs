use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, scale_in, slide_x_in, slide_y_in, slot_id,
    SLOT_BACKDROP, SLOT_FOCUS_SCOPE, SLOT_SURFACE,
};
use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::motion::{MotionTrack, Presence};
use fission_core::op::Fill;
use fission_core::ui::{
    Align, Button, ButtonContent, ButtonVariant, Container, Row, Scroll, SemanticsRegion, Text,
    TextContent, Widget, ZStack,
};
use fission_core::{ActionEnvelope, LayoutDirection, WidgetId};
use fission_ir::{FlexDirection, Role};
use fission_theme::{ButtonHierarchy, ComponentSize, ComponentState};
use serde::{Deserialize, Serialize};
use std::ops::Add;

const MODAL_TITLE_ID_PATH: u32 = 0x71_71_E;
const MODAL_DESCRIPTION_ID_PATH: u32 = 0xDE_5C;
const MODAL_CLOSE_ID_PATH: u32 = 0xC1_05_E;

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

    fn plan(&self, duration_ms: u64, initial_scale: f32) -> ModalMotionPlan {
        let mut plan = ModalMotionPlan::default();
        self.append_plan(&mut plan, duration_ms.max(1), initial_scale);
        plan.normalize()
    }

    fn append_plan(&self, plan: &mut ModalMotionPlan, duration_ms: u64, initial_scale: f32) {
        match self {
            Self::Default => {
                Self::Fade.append_plan(plan, duration_ms, initial_scale);
                Self::Scale.append_plan(plan, duration_ms, initial_scale);
            }
            Self::Fade => {
                push_enter_with_exit(
                    &mut plan.backdrop_enter,
                    &mut plan.backdrop_exit,
                    fade_in(duration_ms),
                );
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    fade_in(duration_ms),
                );
            }
            Self::Scale => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    scale_in(initial_scale.clamp(0.0, 1.0), duration_ms),
                );
            }
            Self::FromTop => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_y_in(-28.0, duration_ms),
                );
            }
            Self::FromBottom => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_y_in(28.0, duration_ms),
                );
            }
            Self::FromLeft => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_x_in(-28.0, duration_ms),
                );
            }
            Self::FromRight => {
                push_enter_with_exit(
                    &mut plan.surface_enter,
                    &mut plan.surface_exit,
                    slide_x_in(28.0, duration_ms),
                );
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(plan, duration_ms, initial_scale);
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
/// * `width` - Optional fixed width. Falls back to the active modal recipe's maximum width.
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
///     backdrop_semantics_identifier: Some("confirm.dismiss-backdrop".into()),
///     close_semantics_identifier: Some("confirm.close".into()),
///     surface_semantics_identifier: Some("confirm.surface".into()),
///     actions: vec![
///         ModalAction { label: "Cancel".into(), on_press: Some(cancel), is_primary: false, semantics_identifier: Some("confirm.cancel".into()) },
///         ModalAction { label: "Delete".into(), on_press: Some(delete), is_primary: true, semantics_identifier: Some("confirm.delete".into()) },
///     ],
///     width: None,
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Modal {
    /// Stable identity used for focus, portal registration, and motion slots.
    pub id: WidgetId,
    /// Dialog heading exposed visually and through semantics.
    pub title: String,
    /// Main dialog body.
    pub content: Widget,
    /// Controlled visibility state.
    pub is_open: bool,
    /// Action dispatched when the backdrop or another dismissal affordance is used.
    pub on_dismiss: Option<ActionEnvelope>,
    /// Stable identifier exposed on the full-screen dismissal backdrop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_semantics_identifier: Option<String>,
    /// Stable identifier exposed on the generated close button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_semantics_identifier: Option<String>,
    /// Stable identifier exposed on the modal surface semantics node.
    ///
    /// When omitted, the historical `fission-modal-surface` identifier is
    /// retained for compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_semantics_identifier: Option<String>,
    /// Footer actions in display order.
    pub actions: Vec<ModalAction>,
    /// Preferred logical width, clamped to the available viewport.
    pub width: Option<f32>,
    /// Optional explicit modal motion. `None` emits no modal-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<ModalMotion>,
}

impl Default for Modal {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("modal"),
            title: String::new(),
            content: fission_core::ui::widgets::spacer::Spacer::default().into(),
            is_open: false,
            on_dismiss: None,
            backdrop_semantics_identifier: None,
            close_semantics_identifier: None,
            surface_semantics_identifier: None,
            actions: Vec::new(),
            width: None,
            motion: None,
        }
    }
}

/// A single action button displayed in the modal footer.
///
/// The legacy `is_primary` flag selects primary or neutral-secondary styling.
/// Use [`ModalFooterAction`] with [`ModalLayout`] when an action needs an
/// explicit button variant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalAction {
    /// Button label displayed in the modal footer.
    pub label: String,
    /// Action dispatched when the button is pressed.
    pub on_press: Option<ActionEnvelope>,
    /// Whether this action receives the primary filled-button treatment.
    pub is_primary: bool,
    /// Stable identifier exposed on this footer button's actionable semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

/// A retained modal-footer action with an explicit visual hierarchy.
///
/// This is the richer action model used by [`ModalFooter`] and
/// [`ModalLayout`]. [`ModalAction`] deliberately retains its original field
/// shape so existing exhaustive struct literals remain source-compatible.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalFooterAction {
    /// Button label displayed in the modal footer.
    pub label: String,
    /// Action dispatched when the button is pressed.
    pub on_press: Option<ActionEnvelope>,
    /// Explicit visual hierarchy for this action.
    pub variant: ButtonVariant,
    /// Stable identifier exposed on this footer button's actionable semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl ModalFooterAction {
    /// Creates a footer action with an explicit visual hierarchy.
    pub fn new(label: impl Into<String>, variant: ButtonVariant) -> Self {
        Self {
            label: label.into(),
            on_press: None,
            variant,
            semantics_identifier: None,
        }
    }
}

impl From<ModalAction> for ModalFooterAction {
    fn from(action: ModalAction) -> Self {
        Self {
            label: action.label,
            on_press: action.on_press,
            variant: if action.is_primary {
                ButtonVariant::Primary
            } else {
                ButtonVariant::SecondaryGray
            },
            semantics_identifier: action.semantics_identifier,
        }
    }
}

/// Retained title anatomy for a modal header.
///
/// The active modal recipe owns typography. `id` is normally assigned by
/// [`ModalLayout`] so the dialog surface can expose a stable `labelled_by`
/// relationship, but it may be supplied when this anatomy is composed in a
/// custom surface.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalTitle {
    /// Localizable title text.
    pub text: TextContent,
    /// Optional custom retained presentation for the title.
    ///
    /// `text` remains the accessible label when this is supplied. When absent,
    /// Fission renders `text` with the active modal title recipe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Optional stable semantic node identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Optional identifier exposed to shells and semantic tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl ModalTitle {
    /// Creates title anatomy from literal or localized text.
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            child: None,
            id: None,
            semantics_identifier: None,
        }
    }

    /// Uses custom retained title presentation while preserving a text label.
    pub fn custom(text: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            text: text.into(),
            child: Some(child.into()),
            id: None,
            semantics_identifier: None,
        }
    }

    /// Assigns a stable semantic identifier.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl Default for ModalTitle {
    fn default() -> Self {
        Self::new(TextContent::default())
    }
}

impl From<ModalTitle> for Widget {
    fn from(title: ModalTitle) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.modal.title_style;
        let tokens = &view.env().theme.tokens;
        let resolved = title.text.resolve(view.env());
        let child = match title.child {
            Some(child) => child,
            None => {
                let mut text = Text::new(title.text)
                    .size(style.font_size.unwrap_or(tokens.typography.font_size_base))
                    .weight(
                        style
                            .font_weight
                            .unwrap_or(tokens.typography.font_weight_medium),
                    )
                    .color(style.text_color.unwrap_or(tokens.colors.text_primary));
                if let Some(line_height) = style.line_height {
                    text = text.line_height(line_height);
                }
                if let Some(letter_spacing) = style.letter_spacing {
                    text = text.letter_spacing(letter_spacing);
                }
                text.into()
            }
        };

        SemanticsRegion {
            id: title.id,
            identifier: title.semantics_identifier,
            label: Some(resolved),
            role: Role::Text,
            focusable: Some(false),
            sequential_focusable: false,
            child: Some(child),
            ..Default::default()
        }
        .into()
    }
}

/// Retained supporting-description anatomy for a modal header.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalDescription {
    /// Localizable description text.
    pub text: TextContent,
    /// Optional custom retained presentation for the description.
    ///
    /// `text` remains the accessible description when this is supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Optional stable semantic node identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Optional identifier exposed to shells and semantic tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl ModalDescription {
    /// Creates description anatomy from literal or localized text.
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            child: None,
            id: None,
            semantics_identifier: None,
        }
    }

    /// Uses custom retained description presentation while preserving text.
    pub fn custom(text: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            text: text.into(),
            child: Some(child.into()),
            id: None,
            semantics_identifier: None,
        }
    }

    /// Assigns a stable semantic identifier.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<ModalDescription> for Widget {
    fn from(description: ModalDescription) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.modal.description_style;
        let tokens = &view.env().theme.tokens;
        let resolved = description.text.resolve(view.env());
        let child = match description.child {
            Some(child) => child,
            None => {
                let mut text = Text::new(description.text)
                    .size(style.font_size.unwrap_or(tokens.typography.font_size_base))
                    .weight(
                        style
                            .font_weight
                            .unwrap_or(tokens.typography.font_weight_regular),
                    )
                    .color(style.text_color.unwrap_or(tokens.colors.text_muted));
                if let Some(line_height) = style.line_height {
                    text = text.line_height(line_height);
                }
                if let Some(letter_spacing) = style.letter_spacing {
                    text = text.letter_spacing(letter_spacing);
                }
                text.into()
            }
        };

        SemanticsRegion {
            id: description.id,
            identifier: description.semantics_identifier,
            label: Some(resolved),
            role: Role::Text,
            focusable: Some(false),
            sequential_focusable: false,
            child: Some(child),
            ..Default::default()
        }
        .into()
    }
}

/// Retained modal header anatomy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalHeader {
    /// Optional retained content at the logical start of the header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leading: Option<Widget>,
    /// Required dialog title.
    pub title: ModalTitle,
    /// Optional supporting text associated with the dialog surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<ModalDescription>,
    /// Optional retained content after the title group and before the close control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing: Option<Widget>,
}

impl ModalHeader {
    /// Creates a header with the required title.
    pub fn new(title: impl Into<TextContent>) -> Self {
        Self {
            leading: None,
            title: ModalTitle::new(title),
            description: None,
            trailing: None,
        }
    }

    /// Adds supporting description text.
    pub fn description(mut self, description: impl Into<TextContent>) -> Self {
        self.description = Some(ModalDescription::new(description));
        self
    }

    /// Adds retained content at the logical start of the header.
    pub fn leading(mut self, child: impl Into<Widget>) -> Self {
        self.leading = Some(child.into());
        self
    }

    /// Adds retained content before the generated close control.
    pub fn trailing(mut self, child: impl Into<Widget>) -> Self {
        self.trailing = Some(child.into());
        self
    }
}

impl From<ModalHeader> for Widget {
    fn from(header: ModalHeader) -> Self {
        ModalHeaderRegion { header }.into()
    }
}

#[derive(Clone, Debug)]
struct ModalHeaderRegion {
    header: ModalHeader,
}

impl From<ModalHeaderRegion> for Widget {
    fn from(region: ModalHeaderRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.modal.header_style;
        let gap = style.gap.unwrap_or(view.env().theme.tokens.spacing.xs);
        let title_stack: Widget = VStack {
            spacing: Some(gap),
            children: std::iter::once(Widget::from(region.header.title))
                .chain(region.header.description.map(Widget::from))
                .collect(),
        }
        .into();
        let mut children = Vec::new();
        if let Some(leading) = region.header.leading {
            children.push(Container::new(leading).flex_shrink(0.0).into());
        }
        children.push(
            Container::new(title_stack)
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .into(),
        );
        if let Some(trailing) = region.header.trailing {
            children.push(Container::new(trailing).flex_shrink(0.0).into());
        }
        let row: Widget = HStack {
            spacing: Some(gap),
            children,
        }
        .into();
        let mut container = Container::new(row)
            .padding(style.padding_box(0.0, 0.0))
            .flex_shrink(0.0);
        if let Some(background) = style.background.clone() {
            container = container.bg_fill(background);
        }
        if let Some(radius) = style.radius {
            container = container.border_radius(radius);
        }
        if let Some(border) = &style.border {
            if let Fill::Solid(color) = &border.fill {
                container = container.border(*color, border.width);
            }
        }
        container.shadows(style.outer_shadows()).into()
    }
}

/// Retained, vertically scrollable modal body anatomy.
///
/// [`ModalLayout`] bounds this region to the available viewport. When its
/// contents exceed that bound, only this body scrolls; the header and footer
/// remain in place.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalContent {
    /// Primary dialog content.
    pub child: Widget,
    /// Whether the body exposes a scrollbar when it overflows.
    #[serde(default = "modal_scrollbar_default")]
    pub show_scrollbar: bool,
}

const fn modal_scrollbar_default() -> bool {
    true
}

impl ModalContent {
    /// Creates a scrollable body around retained content.
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            show_scrollbar: true,
        }
    }
}

impl From<ModalContent> for Widget {
    fn from(content: ModalContent) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.modal.content_style;
        let mut inner = Container::new(content.child).padding(style.padding_box(0.0, 0.0));
        if let Some(background) = style.background.clone() {
            inner = inner.bg_fill(background);
        }
        if let Some(radius) = style.radius {
            inner = inner.border_radius(radius);
        }
        if let Some(border) = &style.border {
            if let Fill::Solid(color) = &border.fill {
                inner = inner.border(*color, border.width);
            }
        }
        inner = inner.shadows(style.outer_shadows());

        Scroll {
            child: Some(inner.into()),
            direction: FlexDirection::Column,
            show_scrollbar: content.show_scrollbar,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}

/// Retained modal action-footer anatomy.
///
/// Content uses the footer recipe's padding while its border is rendered as a
/// separate full-width top boundary. This keeps the footer flush with the
/// dialog surface without adding side or bottom strokes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModalFooter {
    /// Custom retained controls shown before standard actions.
    ///
    /// This is the composition path for controls that do not fit
    /// [`ModalFooterAction`], such as a disclosure, checkbox, or custom action group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Widget>,
    /// Actions in logical desktop display order.
    pub actions: Vec<ModalFooterAction>,
}

/// Full-width top boundary rendered independently from footer content padding.
#[derive(Clone, Debug)]
struct ModalFooterBoundary {
    fill: Fill,
    width: f32,
}

impl From<ModalFooterBoundary> for Widget {
    fn from(boundary: ModalFooterBoundary) -> Self {
        Container::new(Row::default())
            .height(boundary.width)
            .flex_grow(0.0)
            .flex_shrink(0.0)
            .bg_fill(boundary.fill)
            .into()
    }
}

impl ModalFooter {
    /// Creates a footer for the supplied dialog actions.
    pub fn new(actions: Vec<ModalFooterAction>) -> Self {
        Self {
            children: Vec::new(),
            actions,
        }
    }

    /// Creates a footer from arbitrary retained controls.
    pub fn custom(children: Vec<Widget>) -> Self {
        Self {
            children,
            actions: Vec::new(),
        }
    }
}

impl From<ModalFooter> for Widget {
    fn from(footer: ModalFooter) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.modal;
        let style = &theme.footer_style;
        let stacked = view.viewport_size().width.is_finite()
            && view.viewport_size().width < theme.action_stack_breakpoint;
        let gap = style.gap.unwrap_or(view.env().theme.tokens.spacing.s);

        let mut actions: Vec<(bool, Widget)> = footer
            .actions
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                let variant = action.variant;
                let emphasized = matches!(
                    variant,
                    ButtonVariant::Filled | ButtonVariant::Primary | ButtonVariant::Destructive
                );
                let mut button = Button {
                    variant,
                    content: Some(ButtonContent::new(action.label)),
                    on_press: action.on_press,
                    semantics: Some(fission_ir::Semantics {
                        role: Role::Button,
                        focusable: true,
                        sequential_focusable: true,
                        autofocus: index == 0,
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                if let Some(identifier) = action.semantics_identifier {
                    button.id = Some(WidgetId::explicit(&identifier));
                    button = button.semantics_identifier(identifier);
                }
                (emphasized, button.into())
            })
            .collect();

        let action_group: Widget = if stacked {
            actions.sort_by_key(|(emphasized, _)| !*emphasized);
            let mut children: Vec<Widget> = actions.into_iter().map(|(_, action)| action).collect();
            children.extend(footer.children);
            VStack {
                spacing: Some(gap),
                children,
            }
            .into()
        } else {
            let mut children = footer.children;
            children.push(
                fission_core::ui::widgets::spacer::Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
            );
            children.extend(actions.into_iter().map(|(_, action)| action));
            HStack {
                spacing: Some(gap),
                children,
            }
            .into()
        };

        let content: Widget = Container::new(action_group)
            .padding(style.padding_box(0.0, 0.0))
            .flex_shrink(0.0)
            .into();
        let mut children = Vec::new();
        if let Some(border) = &style.border {
            children.push(
                ModalFooterBoundary {
                    fill: border.fill.clone(),
                    width: border.width,
                }
                .into(),
            );
        }
        children.push(content);

        let mut container = Container::new(VStack {
            spacing: Some(0.0),
            children,
        })
        .flex_shrink(0.0);
        if let Some(background) = style.background.clone() {
            container = container.bg_fill(background);
        }
        if let Some(radius) = style.radius {
            container = container.border_radius(radius);
        }
        container.shadows(style.outer_shadows()).into()
    }
}

/// A complete modal surface assembled from retained named anatomy.
///
/// Prefer this when a dialog needs a semantic description or independently
/// retained regions. [`Modal`] remains the compact compatibility API and is
/// lowered through the same recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalLayout {
    /// Stable identity used for focus, portal registration, and motion slots.
    pub id: WidgetId,
    /// Stable header anatomy and accessible title/description.
    pub header: ModalHeader,
    /// Primary scrollable body anatomy.
    pub content: ModalContent,
    /// Optional stable action region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<ModalFooter>,
    /// Controlled visibility state.
    pub is_open: bool,
    /// Action dispatched by backdrop, close control, or semantic dismissal.
    pub on_dismiss: Option<ActionEnvelope>,
    /// Stable identifier exposed on the dismissal backdrop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_semantics_identifier: Option<String>,
    /// Stable identifier exposed on the generated close control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_semantics_identifier: Option<String>,
    /// Localizable accessible label for the generated close control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_label: Option<TextContent>,
    /// Stable identifier exposed on the dialog surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_semantics_identifier: Option<String>,
    /// Preferred logical width, clamped to the available viewport.
    pub width: Option<f32>,
    /// Optional explicit modal motion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<ModalMotion>,
}

impl Default for ModalLayout {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("modal"),
            header: ModalHeader::new(TextContent::default()),
            content: ModalContent::new(fission_core::ui::widgets::spacer::Spacer::default()),
            footer: None,
            is_open: false,
            on_dismiss: None,
            backdrop_semantics_identifier: None,
            close_semantics_identifier: None,
            close_label: None,
            surface_semantics_identifier: None,
            width: None,
            motion: None,
        }
    }
}

impl ModalLayout {
    /// Creates a retained modal layout with its required regions.
    pub fn new(id: WidgetId, title: impl Into<TextContent>, content: impl Into<Widget>) -> Self {
        Self {
            id,
            header: ModalHeader::new(title),
            content: ModalContent::new(content),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug)]
struct ModalRecipe {
    id: WidgetId,
    header: ModalHeader,
    content: ModalContent,
    footer: Option<ModalFooter>,
    is_open: bool,
    on_dismiss: Option<ActionEnvelope>,
    backdrop_semantics_identifier: Option<String>,
    close_semantics_identifier: Option<String>,
    close_label: Option<TextContent>,
    surface_semantics_identifier: Option<String>,
    width: Option<f32>,
    motion: Option<ModalMotion>,
}

impl From<Modal> for Widget {
    fn from(component: Modal) -> Self {
        let footer = (!component.actions.is_empty()).then(|| {
            ModalFooter::new(
                component
                    .actions
                    .into_iter()
                    .map(ModalFooterAction::from)
                    .collect(),
            )
        });
        ModalRecipe {
            id: component.id,
            header: ModalHeader::new(component.title),
            content: ModalContent::new(component.content),
            footer,
            is_open: component.is_open,
            on_dismiss: component.on_dismiss,
            backdrop_semantics_identifier: component.backdrop_semantics_identifier,
            close_semantics_identifier: component.close_semantics_identifier,
            close_label: None,
            surface_semantics_identifier: component.surface_semantics_identifier,
            width: component.width,
            motion: component.motion,
        }
        .into()
    }
}

impl From<ModalLayout> for Widget {
    fn from(component: ModalLayout) -> Self {
        ModalRecipe {
            id: component.id,
            header: component.header,
            content: component.content,
            footer: component.footer,
            is_open: component.is_open,
            on_dismiss: component.on_dismiss,
            backdrop_semantics_identifier: component.backdrop_semantics_identifier,
            close_semantics_identifier: component.close_semantics_identifier,
            close_label: component.close_label,
            surface_semantics_identifier: component.surface_semantics_identifier,
            width: component.width,
            motion: component.motion,
        }
        .into()
    }
}

impl From<ModalRecipe> for Widget {
    fn from(mut component: ModalRecipe) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        if !component.is_open && component.motion.is_none() {
            return fission_core::ui::widgets::spacer::Spacer::default().into();
        }

        let theme = &view.env().theme.components.modal;
        let motion_plan = component
            .motion
            .as_ref()
            .map(|motion| motion.plan(theme.motion_duration_ms, theme.motion_initial_scale));
        let tokens = &view.env().theme.tokens;
        let container_style = &theme.container_style;
        let viewport = view.viewport_size();
        let max_dialog_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - theme.viewport_margin * 2.0).max(0.0)
        } else {
            container_style.max_width.unwrap_or(theme.max_width)
        };
        let dialog_width = component
            .width
            .unwrap_or(container_style.max_width.unwrap_or(theme.max_width))
            .min(max_dialog_width);
        let max_dialog_height = (viewport.height.is_finite() && viewport.height > 0.0)
            .then(|| (viewport.height - theme.viewport_margin * 2.0).max(0.0));

        let backdrop: Widget = Container::new(fission_core::ui::widgets::spacer::Spacer::default())
            .bg_fill(
                theme
                    .scrim_style
                    .background
                    .clone()
                    .unwrap_or_else(|| Fill::Solid(tokens.colors.on_background.with_alpha(26))),
            )
            .backdrop_blur(theme.scrim_blur)
            .flex_grow(1.0)
            .into();
        let mut backdrop_node: Widget = if component.is_open {
            let mut semantics = SemanticsRegion::new(backdrop)
                .focusable(false)
                .sequential_focusable(false);
            semantics.identifier = component.backdrop_semantics_identifier.clone();
            if let Some(on_dismiss) = component.on_dismiss.clone() {
                semantics = semantics.default_action(on_dismiss);
            }
            semantics.into()
        } else {
            backdrop
        };
        if let Some(plan) = &motion_plan {
            backdrop_node = Presence {
                id: slot_id(component.id, SLOT_BACKDROP),
                visible: component.is_open,
                enter: plan.backdrop_enter.clone(),
                exit: plan.backdrop_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: backdrop_node,
                ..Default::default()
            }
            .into();
        }

        let title_id =
            component.header.title.id.unwrap_or_else(|| {
                WidgetId::derived(component.id.as_u128(), &[MODAL_TITLE_ID_PATH])
            });
        component.header.title.id = Some(title_id);
        let accessible_label = component.header.title.text.resolve(view.env());
        let description_id = component.header.description.as_mut().map(|description| {
            let id = description.id.unwrap_or_else(|| {
                WidgetId::derived(component.id.as_u128(), &[MODAL_DESCRIPTION_ID_PATH])
            });
            description.id = Some(id);
            id
        });

        let close_control = component.on_dismiss.clone().map(|on_dismiss| {
            let style = &theme.close_button_style;
            let fallback_style = view.env().theme.components.button.resolve(
                ButtonHierarchy::TertiaryGray,
                ComponentSize::Sm,
                ComponentState::Default,
            );
            let fallback_control_size = fallback_style
                .height
                .or(fallback_style.width)
                .unwrap_or(tokens.spacing.xl);
            let close_label = component
                .close_label
                .clone()
                .unwrap_or_else(|| TextContent::KeyWithFallback {
                    key: "fission.modal.close".into(),
                    fallback: "Close".into(),
                })
                .resolve(view.env());
            let mut close_button = Button {
                id: Some(WidgetId::derived(
                    component.id.as_u128(),
                    &[MODAL_CLOSE_ID_PATH],
                )),
                variant: ButtonVariant::Ghost,
                semantics: Some(fission_ir::Semantics {
                    role: Role::Button,
                    label: Some(close_label),
                    focusable: true,
                    sequential_focusable: true,
                    ..Default::default()
                }),
                child: Some(
                    Icon::svg(fission_icons::material::navigation::close::regular())
                        .size(
                            style
                                .icon_size
                                .or(fallback_style.icon_size)
                                .unwrap_or(tokens.spacing.m),
                        )
                        .into(),
                ),
                on_press: Some(on_dismiss),
                width: Some(
                    style
                        .width
                        .or(style.height)
                        .unwrap_or(fallback_control_size),
                ),
                height: Some(
                    style
                        .height
                        .or(style.width)
                        .unwrap_or(fallback_control_size),
                ),
                padding: Some(style.padding_box(0.0, 0.0)),
                size: ComponentSize::Sm,
                ..Default::default()
            };
            if let Some(identifier) = &component.close_semantics_identifier {
                close_button = close_button.semantics_identifier(identifier.clone());
            }
            Widget::from(close_button)
        });

        let mut main_children = vec![Widget::from(ModalHeaderRegion {
            header: component.header,
        })];
        main_children.push(component.content.into());
        let footer = component
            .footer
            .filter(|footer| !footer.actions.is_empty() || !footer.children.is_empty());
        let mut main_padding = container_style.padding.unwrap_or([tokens.spacing.m; 4]);
        if footer.is_some() {
            main_padding[3] = 0.0;
        }
        let main_region: Widget = Container::new(VStack {
            spacing: Some(container_style.gap.unwrap_or(tokens.spacing.m)),
            children: main_children,
        })
        .padding(main_padding)
        .into();
        let mut surface_children = vec![main_region];
        if let Some(footer) = footer {
            surface_children.push(footer.into());
        }
        let flow: Widget = VStack {
            spacing: Some(container_style.gap.unwrap_or(tokens.spacing.m)),
            children: surface_children,
        }
        .into();
        let surface_content: Widget = if let Some(close_control) = close_control {
            let close_style = &theme.close_button_style;
            let inset_top = close_style.inset_top.unwrap_or(tokens.spacing.s);
            let inset_end = close_style.inset_end.unwrap_or(tokens.spacing.s);
            let (left, right) = if view.env().layout_direction == LayoutDirection::RightToLeft {
                (Some(inset_end), None)
            } else {
                (None, Some(inset_end))
            };
            ZStack {
                children: vec![
                    flow,
                    fission_core::ui::Positioned {
                        left,
                        right,
                        top: Some(inset_top),
                        child: Some(close_control),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into()
        } else {
            flow
        };

        let mut modal_card_builder = Container::new(surface_content)
            .bg_fill(
                container_style
                    .background
                    .clone()
                    .unwrap_or(Fill::Solid(theme.bg_color)),
            )
            .border_radius(container_style.radius.unwrap_or(theme.radius))
            .shadows(container_style.outer_shadows())
            .width(dialog_width)
            .clip_overflow(true);
        if let Some(max_dialog_height) = max_dialog_height {
            modal_card_builder = modal_card_builder.max_height(max_dialog_height);
        }
        if let Some(border) = container_style.border.as_ref() {
            if let Fill::Solid(color) = &border.fill {
                modal_card_builder = modal_card_builder.border(*color, border.width);
            }
        }
        if let Some(dash) = &container_style.border_dash {
            modal_card_builder = modal_card_builder.border_dash(dash.clone());
        }
        if container_style.shadows.is_empty() {
            if let Some(shadow) = theme.shadow {
                modal_card_builder = modal_card_builder.shadow(shadow);
            }
        }

        let mut modal_semantics = SemanticsRegion::new(modal_card_builder)
            .identifier(
                component
                    .surface_semantics_identifier
                    .unwrap_or_else(|| "fission-modal-surface".into()),
            )
            .role(Role::Dialog)
            .label(accessible_label)
            .labelled_by(vec![title_id])
            .modal(true)
            // A dialog with static body content still needs a valid focus
            // entry target. Keep the surface programmatically focusable while
            // excluding it from ordinary sequential traversal whenever real
            // controls are present.
            .focusable(true)
            .sequential_focusable(false);
        if let Some(description_id) = description_id {
            modal_semantics = modal_semantics.described_by(vec![description_id]);
        }
        if component.is_open {
            if let Some(on_dismiss) = component.on_dismiss.clone() {
                modal_semantics = modal_semantics.dismiss_action(on_dismiss);
            }
        }
        let mut modal_card: Widget = modal_semantics.into();
        if let Some(plan) = &motion_plan {
            modal_card = Presence {
                id: slot_id(component.id, SLOT_SURFACE),
                visible: component.is_open,
                enter: plan.surface_enter.clone(),
                exit: plan.surface_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: modal_card,
                ..Default::default()
            }
            .into();
        }

        let center_layer: Widget = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Align::new(modal_card).into()),
            ..Default::default()
        }
        .into();
        let surface_layer = if component.is_open {
            fission_core::ui::widgets::FocusScope {
                id: Some(slot_id(component.id, SLOT_FOCUS_SCOPE)),
                is_barrier: true,
                children: vec![center_layer],
            }
            .into()
        } else {
            center_layer
        };

        let root = Container::new(ZStack {
            children: vec![
                fission_core::ui::Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop_node),
                    ..Default::default()
                }
                .into(),
                surface_layer,
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
            Some(component.id),
            positioned_root,
        );

        fission_core::ui::widgets::spacer::Spacer::default().into()
    }
}
