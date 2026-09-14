use fission_core::op::{AlignItems, Length, TextAlign};
use fission_core::ui::{Column, Container, SemanticsRegion, Text, Widget, WidgetKind};
use fission_ir::op::Fill;
use fission_ir::Role;
use serde::{Deserialize, Serialize};

/// A centered placeholder displayed when a view has no content.
///
/// The bordered panel groups an optional icon, a title, an optional
/// description, and an optional action. Its message regions remain bounded and
/// centered while the surface fills the available parent width.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyState {
    /// Optional illustration or icon shown above the title.
    pub icon: Option<Widget>,
    /// Primary explanation of the empty state.
    pub title: String,
    /// Optional supporting guidance.
    pub description: Option<String>,
    /// Optional action widget, commonly a button that creates the first item.
    pub action: Option<Widget>,
}

impl From<EmptyState> for Widget {
    fn from(component: EmptyState) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.empty_state;
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let mut header = Vec::new();

        if let Some(icon) = component.icon {
            let style = &theme.icon_style;
            let icon_recipe = match icon.kind() {
                WidgetKind::Icon(icon) => Some(icon.clone()),
                _ => None,
            };
            let icon = if let Some(mut icon) = icon_recipe {
                if icon.size.is_none() {
                    icon.size = Some(style.icon_size.unwrap_or(tokens.spacing.m));
                }
                if icon.color.is_none() {
                    icon.color = Some(style.text_color.unwrap_or(tokens.colors.text_secondary));
                }
                icon.into()
            } else {
                icon
            };
            let mut region = Container::new(icon)
                .width(style.width.unwrap_or(32.0))
                .height(style.height.unwrap_or(32.0))
                .align_child(fission_core::op::BoxAlignment::Center)
                .margin([0.0, 0.0, 0.0, style.margin_bottom.unwrap_or(0.0)]);
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
            header.push(region.into());
        }

        let title_style = &theme.title_style;
        header.push(
            Text::new(component.title.clone())
                .size(title_style.font_size.unwrap_or(typography.font_size_base))
                .weight(
                    title_style
                        .font_weight
                        .unwrap_or(typography.font_weight_medium),
                )
                .line_height(
                    title_style
                        .line_height
                        .unwrap_or(typography.font_size_base * typography.line_height_snug),
                )
                .color(title_style.text_color.unwrap_or(tokens.colors.text_primary))
                .text_align(TextAlign::Center)
                .into(),
        );

        if let Some(description) = component.description {
            let style = &theme.description_style;
            header.push(
                Text::new(description)
                    .size(style.font_size.unwrap_or(typography.font_size_base))
                    .weight(style.font_weight.unwrap_or(typography.font_weight_regular))
                    .line_height(
                        style
                            .line_height
                            .unwrap_or(typography.font_size_base * typography.line_height_relaxed),
                    )
                    .color(style.text_color.unwrap_or(tokens.colors.text_secondary))
                    .text_align(TextAlign::Center)
                    .into(),
            );
        }

        let header_style = &theme.header_style;
        let mut header_region = Container::new(Column {
            gap: header_style.gap.or(Some(tokens.spacing.s)),
            align_items: AlignItems::Center,
            children: header,
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .max_width(header_style.max_width.unwrap_or(384.0));
        if let Some(padding) = header_style.padding {
            header_region = header_region.padding(padding);
        }

        let mut sections = vec![header_region.into()];
        if let Some(action) = component.action {
            let style = &theme.action_style;
            let mut region = Container::new(action);
            if let Some(width) = style.width {
                region = region.width(width);
            } else {
                region = region.width_length(Length::percent(100.0));
            }
            if let Some(max_width) = style.max_width {
                region = region.max_width(max_width);
            }
            if let Some(padding) = style.padding {
                region = region.padding(padding);
            }
            sections.push(
                region
                    .align_child(fission_core::op::BoxAlignment::Center)
                    .into(),
            );
        }

        let style = theme.resolve_surface(view.viewport_size().width);
        let mut surface = Container::new(Column {
            gap: style.gap.or(Some(tokens.spacing.m)),
            align_items: AlignItems::Center,
            children: sections,
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding(style.padding_box(tokens.spacing.l, tokens.spacing.l))
        .border_radius(style.radius.unwrap_or(tokens.radii.large));
        if let Some(min_height) = style.min_height {
            surface = surface.min_height(min_height);
        }
        if let Some(max_width) = style.max_width {
            surface = surface.max_width(max_width);
        }
        if let Some(background) = style.background.clone() {
            surface = surface.bg_fill(background);
        }
        if let Some(border) = &style.border {
            if let Fill::Solid(color) = &border.fill {
                surface = surface.border(*color, border.width);
            }
        }
        if let Some(dash) = &style.border_dash {
            surface = surface.border_dash(dash.clone());
        }

        // An empty region has to say why it is empty; otherwise a reader meets
        // silence and cannot tell the content failed to load from there being
        // none.
        SemanticsRegion::new(surface)
            .role(Role::Status)
            .label(component.title.clone())
            .into()
    }
}
