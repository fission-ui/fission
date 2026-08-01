use super::TargetChip;
use crate::catalog::{ExampleCategory, ExampleDefinition};
use crate::i18n::message;
use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::icons::material;
use fission::op::{AlignItems, Fill, FlexWrap};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct ExampleRow {
    pub(super) example: ExampleDefinition,
    pub(super) selected: bool,
}

impl From<ExampleRow> for Widget {
    fn from(component: ExampleRow) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let path = format!("/examples/{}", component.example.slug);
        let navigate = with_reducer!(ctx, Navigate(path), on_navigate);
        let title = message(view.env(), component.example.title_key);
        let target_chips = component
            .example
            .targets
            .iter()
            .take(3)
            .map(|target| TargetChip { target: *target }.into())
            .collect::<Vec<_>>();

        Pressable::new(Row {
            children: widgets![
                Icon::svg(match component.example.category {
                    ExampleCategory::Start => material::av::play_arrow::round(),
                    ExampleCategory::Apps => material::action::dashboard::round(),
                    ExampleCategory::Galleries => material::device::widgets::round(),
                    ExampleCategory::Platform => material::device::devices::round(),
                    ExampleCategory::Diagnostics => material::action::bug_report::round(),
                })
                .size(tokens.typography.font_size_xl)
                .color(if component.selected {
                    tokens.colors.primary
                } else {
                    tokens.colors.text_muted
                }),
                Column {
                    children: widgets![
                        Text::new(TextContent::Key(component.example.title_key.into()))
                            .size(tokens.typography.label_large_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading),
                        Text::new(TextContent::Key(component.example.summary_key.into()))
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_muted),
                        Row {
                            children: target_chips,
                            gap: Some(tokens.spacing.xs),
                            wrap: FlexWrap::Wrap,
                            ..Default::default()
                        },
                    ],
                    gap: Some(tokens.spacing.xs),
                    flex_grow: 1.0,
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .id(WidgetId::explicit(&format!(
            "showcase.catalog.row.{}",
            component.example.slug
        )))
        .on_press(navigate)
        .role(PressableRole::Link)
        .label(title)
        .semantics_identifier(format!("showcase.example.{}", component.example.slug))
        .style(PressableStyle {
            padding: Some(Length::all(Length::points(tokens.spacing.m))),
            corner_radius: Some(tokens.radii.large),
            background: component
                .selected
                .then(|| Fill::Solid(tokens.colors.primary_subtle)),
            ..Default::default()
        })
        .hover(PressableStyle {
            background: Some(Fill::Solid(tokens.colors.surface_sunken)),
            ..Default::default()
        })
        .into()
    }
}
