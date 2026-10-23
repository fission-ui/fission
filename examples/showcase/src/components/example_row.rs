use crate::catalog::ExampleDefinition;
use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::op::{AlignItems, BoxAlignment, Fill};
use fission::prelude::*;

fission_component! {
/// One example in the catalog: its name and, quietly, where it runs.
///
/// The summary lives in the workbench header, so a row stays one line and more
/// of the catalog fits on screen.
    #[derive(Clone, Copy, Debug)] pub(super) ExampleRow<ShowcaseState> {
        example: ExampleDefinition,
        selected: bool,
    }
    |component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let path = format!("/examples/{}", component.example.slug);
        let navigate = with_reducer!(cx.build, Navigate(path), on_navigate);
        let title = cx.view.env().tr(component.example.title_key);
        let platforms = match component.example.targets {
            [only] => only.label().to_string(),
            targets => cx.view
                .tr("showcase.catalog.platform_count")
                .replace("{count}", &targets.len().to_string()),
        };
        // A narrow window is likely touch, so rows meet the touch target there.
        let min_height = if cx.view.viewport_size().width < tokens.breakpoints.medium_max {
            tokens.sizing.min_touch_target
        } else {
            tokens
                .sizing
                .control_md
                .max(tokens.sizing.min_pointer_target)
        };

    } =>
        Pressable(
            Container(Row(
                children: [
                    Text(TextContent::Key(component.example.title_key.into()))
                        .size(tokens.typography.label_large_size)
                        .weight(if component.selected {
                            tokens.typography.font_weight_semibold
                        } else {
                            tokens.typography.font_weight_medium
                        })
                        .color(tokens.colors.text_primary)
                        .max_lines(1),
                    Spacer(flex_grow: 1.0_f32),
                    Text(platforms)
                        .size(tokens.typography.font_size_xs)
                        .color(if component.selected {
                            tokens.colors.text_secondary
                        } else {
                            tokens.colors.text_muted
                        })
                        .max_lines(1),
                ],
                gap: tokens.spacing.m,
                align_items: AlignItems::Center,
            ))
                .width_length(Length::percent(100.0))
                .min_height(min_height)
                .align_child(BoxAlignment::Stretch),
        )
        .id(WidgetId::explicit(&format!(
            "showcase.catalog.row.{}",
            component.example.slug
        )))
        .on_press(navigate)
        .role(PressableRole::Link)
        .label(title)
        .semantics_identifier(format!("showcase.example.{}", component.example.slug))
        .style(PressableStyle {
            padding: Some(Length::symmetric(
                Length::points(tokens.spacing.m),
                Length::points(tokens.spacing.xs),
            )),
            corner_radius: Some(tokens.radii.medium),
            background: component
                .selected
                .then(|| Fill::Solid(tokens.colors.primary_subtle)),
            ..Default::default()
        })
        // Hovering the open example keeps its highlight instead of replacing it.
        .hover(PressableStyle {
            background: Some(Fill::Solid(if component.selected {
                tokens.colors.primary_subtle
            } else {
                tokens.colors.muted()
            })),
            ..Default::default()
        })
}
