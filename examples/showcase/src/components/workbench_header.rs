use super::TargetChip;
use crate::catalog::ExampleDefinition;
use crate::semantics::ShowcaseSemantics;
use crate::state::{on_open_source, OpenSource, ShowcaseState};
use fission::icons::material;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;

fission_component! {
/// The open example's name, platforms, summary, run command and source link.
    #[derive(Clone, Copy, Debug)] pub(crate) WorkbenchHeader<ShowcaseState> {
        example: ExampleDefinition,
    }
    |component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let source = with_reducer!(
            cx.build,
            OpenSource(component.example.source_url()),
            on_open_source
        );
    } =>
        Row(
            children: [
                Column(
                    children: [
                        Row(
                            children: [
                                Text::new(TextContent::Key(component.example.title_key.into()))
                                    .size(typography.font_size_xl)
                                    .weight(typography.font_weight_semibold)
                                    .color(tokens.colors.text_primary),
                                Row(
                                    children: [
                                        for target in component.example.targets.iter().copied() {
                                            TargetChip(target, key: target.label())
                                        },
                                    ],
                                    gap: tokens.spacing.xs,
                                    wrap: FlexWrap::Wrap,
                                ),
                            ],
                            gap: tokens.spacing.s,
                            wrap: FlexWrap::Wrap,
                            align_items: AlignItems::Center,
                        ),
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
                    gap: tokens.spacing.xs,
                    flex_grow: 1.0_f32,
                    flex_shrink: 1.0_f32,
                ),
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Sm,
                    content: ButtonContent::new(TextContent::Key("showcase.workbench.source".into()))
                        .leading_icon(Icon::svg(material::action::source::round())),
                    on_press: source,
                    semantics: Semantics::link(cx.view.env().tr("showcase.workbench.source"))
                        .identifier(format!("showcase.source.{}", component.example.slug)),
                ),
            ],
            gap: tokens.spacing.l,
            align_items: AlignItems::Center,
        )
}
