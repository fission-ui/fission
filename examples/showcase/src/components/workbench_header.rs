use super::TargetChip;
use crate::catalog::ExampleDefinition;
use crate::semantics::ShowcaseSemantics;
use crate::state::{on_open_source, OpenSource, ShowcaseState};
use fission::icons::material;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;

/// The open example's name, platforms, summary, run command and source link.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkbenchHeader {
    pub(crate) example: ExampleDefinition,
}

impl From<WorkbenchHeader> for Widget {
    fn from(component: WorkbenchHeader) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let source = with_reducer!(
            ctx,
            OpenSource(component.example.source_url()),
            on_open_source
        );
        let targets = component
            .example
            .targets
            .iter()
            .map(|target| TargetChip { target: *target }.into())
            .collect();

        Row {
            children: widgets![
                Column {
                    children: widgets![
                        Row {
                            children: widgets![
                                Text::new(TextContent::Key(component.example.title_key.into()))
                                    .size(typography.font_size_xl)
                                    .weight(typography.font_weight_semibold)
                                    .color(tokens.colors.text_primary),
                                Row {
                                    children: targets,
                                    gap: Some(tokens.spacing.xs),
                                    wrap: FlexWrap::Wrap,
                                    ..Default::default()
                                },
                            ],
                            gap: Some(tokens.spacing.s),
                            wrap: FlexWrap::Wrap,
                            align_items: AlignItems::Center,
                            ..Default::default()
                        },
                        Text::new(TextContent::Key(component.example.summary_key.into()))
                            .size(typography.body_medium_size)
                            .color(tokens.colors.text_secondary)
                            .wrap(true),
                        Text::new(format!(
                            "{} · {}",
                            component.example.package, component.example.command
                        ))
                        .size(typography.font_size_xs)
                        .color(tokens.colors.text_muted)
                        .selectable(true),
                    ],
                    gap: Some(tokens.spacing.xs),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Sm,
                    content: Some(
                        ButtonContent::new(TextContent::Key("showcase.workbench.source".into()))
                            .leading_icon(Icon::svg(material::action::source::round())),
                    ),
                    on_press: Some(source),
                    semantics: Some(
                        Semantics::link(view.env().tr("showcase.workbench.source"))
                            .identifier(format!("showcase.source.{}", component.example.slug)),
                    ),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.l),
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into()
    }
}
