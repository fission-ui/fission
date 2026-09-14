use crate::Icon;
use fission_core::op::{AlignItems, Fill};
use fission_core::ui::{
    Column, Composite, Container, Positioned, Row, SemanticsRegion, Text, TextContent, Widget,
    ZStack,
};
use fission_icons::material;
use fission_ir::Role;
use fission_theme::{AlertTheme, ResolvedComponentStyle};
use serde::{Deserialize, Serialize};

/// Semantic tone controlling an [`Alert`]'s icon and color treatment.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AlertKind {
    /// Neutral information.
    Info,
    /// Potential problem requiring attention.
    Warning,
    /// Failed or invalid state.
    Error,
    /// Successful or confirmed state.
    Success,
}

/// Persistent inline message with semantic tone, title, and optional detail.
///
/// This convenience API lowers through the same named anatomy as
/// [`AlertLayout`]. Use that layout directly when an alert needs custom
/// retained content, a custom leading visual, or a trailing action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    /// Tone used to resolve icon and design-system colors.
    pub kind: AlertKind,
    /// Concise primary message.
    pub title: String,
    /// Optional supporting explanation.
    pub description: Option<String>,
}

impl From<Alert> for Widget {
    fn from(component: Alert) -> Self {
        let mut content = vec![AlertTitle::new(component.title).into()];
        if let Some(description) = component.description {
            content.push(AlertDescription::new(description).into());
        }

        AlertLayout::new(component.kind, AlertContent::new(content)).into()
    }
}

/// Leading visual displayed by [`AlertLayout`].
///
/// The default icon is selected from the alert tone and receives the active
/// design-system icon recipe. A custom widget keeps its own retained styling.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum AlertLeading {
    /// Use the design-system-sized icon associated with the alert tone.
    #[default]
    Default,
    /// Do not render a leading visual.
    Hidden,
    /// Render caller-provided retained content.
    Custom(Widget),
}

/// Complete retained alert anatomy.
///
/// The surface, tone, leading visual, content region, and optional trailing
/// action remain distinct so applications can compose richer alerts without
/// rebuilding the component from generic containers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlertLayout {
    /// Semantic and visual tone.
    pub kind: AlertKind,
    /// Default, hidden, or caller-provided leading visual.
    pub leading: AlertLeading,
    /// Main retained content, commonly [`AlertContent`].
    pub content: Widget,
    /// Optional trailing control or action group.
    pub action: Option<Widget>,
}

impl AlertLayout {
    pub fn new(kind: AlertKind, content: impl Into<Widget>) -> Self {
        Self {
            kind,
            leading: AlertLeading::Default,
            content: content.into(),
            action: None,
        }
    }

    /// Replaces the tone-derived leading icon with retained content.
    pub fn leading(mut self, leading: impl Into<Widget>) -> Self {
        self.leading = AlertLeading::Custom(leading.into());
        self
    }

    /// Omits the leading visual while preserving content alignment.
    pub fn without_leading(mut self) -> Self {
        self.leading = AlertLeading::Hidden;
        self
    }

    /// Adds a trailing control or action group.
    pub fn action(mut self, action: impl Into<Widget>) -> Self {
        self.action = Some(action.into());
        self
    }
}

impl From<AlertLayout> for Widget {
    fn from(component: AlertLayout) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.alert;
        let tokens = &view.env().theme.tokens;
        let fallback_tone = match component.kind {
            AlertKind::Info => tokens.colors.info,
            AlertKind::Warning => tokens.colors.warning,
            AlertKind::Error => tokens.colors.error,
            AlertKind::Success => tokens.colors.success,
        };
        let tone = resolved_tone_style(theme, component.kind, fallback_tone);
        let style = theme.surface_style.merge(&tone);

        let mut children = Vec::new();
        match component.leading {
            AlertLeading::Default => {
                let icon = match component.kind {
                    AlertKind::Info => material::action::info::regular(),
                    AlertKind::Warning => material::alert::warning::regular(),
                    AlertKind::Error => material::alert::error::regular(),
                    AlertKind::Success => material::action::check_circle::regular(),
                };
                let mut leading: Composite = Composite::new(
                    Icon::svg(icon)
                        .size(theme.icon_style.icon_size.unwrap_or(tokens.spacing.m))
                        .color(tone.text_color.unwrap_or(tokens.colors.text_primary)),
                );
                if let Some(translate_y) = theme.icon_style.translate_y {
                    leading = leading.translate_y(translate_y);
                }
                children.push(
                    Container::new(leading)
                        .margin([0.0, 0.0, theme.icon_style.inset_top.unwrap_or(0.0), 0.0])
                        .into(),
                );
            }
            AlertLeading::Hidden => {}
            AlertLeading::Custom(leading) => {
                let mut leading = Composite::new(leading);
                if let Some(translate_y) = theme.icon_style.translate_y {
                    leading = leading.translate_y(translate_y);
                }
                children.push(
                    Container::new(leading)
                        .margin([0.0, 0.0, theme.icon_style.inset_top.unwrap_or(0.0), 0.0])
                        .into(),
                );
            }
        }

        children.push(
            Container::new(component.content)
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .into(),
        );
        let action = component.action;
        // Logical [start, end, top, bottom]: the trailing action sits at the
        // end of the row, whichever physical edge that is.
        let mut padding = style.padding_box(tokens.spacing.s, tokens.spacing.s);
        if action.is_some() {
            let reserve = theme.action_style.width.unwrap_or(64.0)
                + theme.action_style.inset_end.unwrap_or(tokens.spacing.s);
            padding[1] = padding[1].max(reserve);
        }
        let row: Widget = Row {
            gap: style.gap,
            align_items: AlignItems::Start,
            children,
            ..Default::default()
        }
        .into();
        let mut layers = vec![row];
        if let Some(action) = action {
            let action_style = &theme.action_style;
            layers.push(
                Positioned {
                    end: Some(action_style.inset_end.unwrap_or(8.0)),
                    top: Some(action_style.inset_top.unwrap_or(8.0)),
                    width: action_style.width,
                    child: Some(AlertActionRegion { child: action }.into()),
                    ..Default::default()
                }
                .into(),
            );
        }

        let mut surface = Container::new(ZStack {
            children: layers,
            ..Default::default()
        })
        .bg_fill(
            style
                .background
                .clone()
                .unwrap_or(Fill::Solid(tokens.colors.surface)),
        )
        .border_radius(style.radius.unwrap_or(theme.radius))
        .padding(padding)
        .shadows(style.outer_shadows());
        if let Some(min_height) = style.min_height {
            surface = surface.min_height(min_height);
        }
        if let Some(border) = &style.border {
            if let Fill::Solid(color) = &border.fill {
                surface = surface.border(*color, border.width);
            }
        }

        SemanticsRegion::new(surface).role(Role::Alert).into()
    }
}

/// Vertically arranged retained content within an [`AlertLayout`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlertContent {
    /// Standard title, description, or custom retained content.
    pub children: Vec<Widget>,
}

impl AlertContent {
    pub fn new(children: Vec<Widget>) -> Self {
        Self { children }
    }
}

impl From<AlertContent> for Widget {
    fn from(component: AlertContent) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.alert.content_style;
        Column {
            gap: style.gap,
            children: component.children,
            ..Default::default()
        }
        .into()
    }
}

/// Primary alert heading using the active alert typography recipe.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlertTitle {
    /// Localized or literal title content.
    pub text: TextContent,
}

impl AlertTitle {
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self { text: text.into() }
    }
}

impl From<AlertTitle> for Widget {
    fn from(component: AlertTitle) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.alert.title_style;
        let typography = &view.env().theme.tokens.typography;

        Text::new(component.text)
            .size(style.font_size.unwrap_or(typography.font_size_base))
            .weight(style.font_weight.unwrap_or(typography.font_weight_medium))
            .line_height(style.line_height.unwrap_or(typography.font_size_base))
            .color(
                style
                    .text_color
                    .unwrap_or(view.env().theme.tokens.colors.text_primary),
            )
            .into()
    }
}

/// Supporting alert copy using the active alert typography recipe.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlertDescription {
    /// Localized or literal supporting content.
    pub text: TextContent,
}

impl AlertDescription {
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self { text: text.into() }
    }
}

impl From<AlertDescription> for Widget {
    fn from(component: AlertDescription) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.alert.description_style;
        let typography = &view.env().theme.tokens.typography;

        Text::new(component.text)
            .size(style.font_size.unwrap_or(typography.font_size_base))
            .weight(style.font_weight.unwrap_or(typography.font_weight_regular))
            .line_height(style.line_height.unwrap_or(typography.font_size_base))
            .color(
                style
                    .text_color
                    .unwrap_or(view.env().theme.tokens.colors.text_secondary),
            )
            .into()
    }
}

#[derive(Clone, Debug)]
struct AlertActionRegion {
    child: Widget,
}

impl From<AlertActionRegion> for Widget {
    fn from(component: AlertActionRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.alert.action_style;
        let mut region = Container::new(component.child)
            .padding(style.padding_box(0.0, 0.0))
            .shadows(style.outer_shadows());
        if let Some(width) = style.width {
            region = region.width(width);
        }
        if let Some(background) = style.background.clone() {
            region = region.bg_fill(background);
        }
        if let Some(radius) = style.radius {
            region = region.border_radius(radius);
        }
        if let Some(border) = &style.border {
            if let Fill::Solid(color) = &border.fill {
                region = region.border(*color, border.width);
            }
        }
        region.into()
    }
}

fn resolved_tone_style(
    theme: &AlertTheme,
    kind: AlertKind,
    fallback_text: fission_ir::op::Color,
) -> ResolvedComponentStyle {
    let (style, compatibility_background) = match kind {
        AlertKind::Info => (&theme.info_style, theme.info_bg),
        AlertKind::Warning => (&theme.warning_style, theme.warning_bg),
        AlertKind::Error => (&theme.error_style, theme.error_bg),
        AlertKind::Success => (&theme.success_style, theme.success_bg),
    };
    let mut style = style.clone();
    style
        .background
        .get_or_insert(Fill::Solid(compatibility_background));
    style.text_color.get_or_insert(fallback_text);
    style
}
