use super::TargetChip;
use crate::catalog::ExampleDefinition;
use crate::i18n::message;
use crate::semantics::ShowcaseSemantics;
use crate::state::{on_open_source, OpenSource, ShowcaseState};
use fission::icons::material;
use fission::op::{AlignItems, FlexWrap, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkbenchHeader {
    pub(crate) example: ExampleDefinition,
}

impl From<WorkbenchHeader> for Widget {
    fn from(component: WorkbenchHeader) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
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
                                    .size(tokens.typography.heading_size)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.heading),
                                Row {
                                    children: targets,
                                    gap: Some(tokens.spacing.xs),
                                    wrap: FlexWrap::Wrap,
                                    ..Default::default()
                                },
                            ],
                            gap: Some(tokens.spacing.m),
                            wrap: FlexWrap::Wrap,
                            align_items: AlignItems::Center,
                            ..Default::default()
                        },
                        Text::new(TextContent::Key(component.example.summary_key.into()))
                            .size(tokens.typography.body_medium_size)
                            .color(tokens.colors.text_secondary),
                        Text::new(format!(
                            "{} · {}",
                            component.example.package, component.example.command
                        ))
                        .size(tokens.typography.font_size_xs)
                        .color(tokens.colors.text_muted)
                        .selectable(true),
                    ],
                    gap: Some(tokens.spacing.xs),
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::SecondaryGray,
                    size: ComponentSize::Sm,
                    child: Some(
                        Row {
                            children: widgets![
                                Icon::svg(material::action::source::round())
                                    .size(tokens.typography.font_size_lg),
                                Text::new(TextContent::Key("showcase.workbench.source".into())),
                            ],
                            gap: Some(tokens.spacing.s),
                            align_items: AlignItems::Center,
                            ..Default::default()
                        }
                        .into(),
                    ),
                    on_press: Some(source),
                    semantics: Some(
                        Semantics::link(message(view.env(), "showcase.workbench.source"))
                            .identifier(format!("showcase.source.{}", component.example.slug),),
                    ),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.l),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
            ..Default::default()
        }
        .into()
    }
}
