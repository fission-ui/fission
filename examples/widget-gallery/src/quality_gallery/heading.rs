use fission::prelude::*;

fission_component! {
    pub(super) QualityHeading<()> { compact: bool }
    |heading, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Row(
            align_items: fission::op::AlignItems::End,
            children: [
                Column(
                    gap: tokens.spacing.xs,
                    children: [
                        Text("Personal account")
                            .size(tokens.typography.font_size_xs)
                            .line_height(16.0)
                            .weight(tokens.typography.font_weight_medium)
                            .color(tokens.colors.text_secondary),
                        Text("Settings")
                            .size(if heading.compact {
                                tokens.typography.font_size_lg
                            } else {
                                tokens.typography.font_size_xl
                            })
                            .line_height(if heading.compact { 24.0 } else { 28.0 })
                            .weight(tokens.typography.font_weight_semibold)
                            .color(tokens.colors.heading),
                        Text("Manage how your account appears and behaves.")
                            .size(tokens.typography.font_size_base)
                            .line_height(20.0)
                            .color(tokens.colors.text_secondary),
                    ],
                ),
                Spacer(flex_grow: 1.0_f32),
                if heading.compact {
                    Spacer()
                } else {
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            Icon::svg(material::navigation::check::regular())
                                .size(14.0)
                                .color(tokens.colors.text_secondary),
                            Text("All changes saved")
                                .size(tokens.typography.font_size_xs)
                                .line_height(16.0)
                                .color(tokens.colors.text_secondary),
                        ],
                    )
                },
            ],
        )
}
