use fission::prelude::*;

pub(super) struct QualityHeading {
    pub compact: bool,
}

impl From<QualityHeading> for Widget {
    fn from(heading: QualityHeading) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;

        Row {
            align_items: fission::op::AlignItems::End,
            children: widgets![
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: widgets![
                        Text::new("Personal account")
                            .size(tokens.typography.font_size_xs)
                            .line_height(16.0)
                            .weight(tokens.typography.font_weight_medium)
                            .color(tokens.colors.text_secondary),
                        Text::new("Settings")
                            .size(if heading.compact {
                                tokens.typography.font_size_lg
                            } else {
                                tokens.typography.font_size_xl
                            })
                            .line_height(if heading.compact { 24.0 } else { 28.0 })
                            .weight(tokens.typography.font_weight_semibold)
                            .color(tokens.colors.heading),
                        Text::new("Manage how your account appears and behaves.")
                            .size(tokens.typography.font_size_base)
                            .line_height(20.0)
                            .color(tokens.colors.text_secondary),
                    ],
                    ..Default::default()
                },
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                if heading.compact {
                    Widget::from(Spacer::default())
                } else {
                    Row {
                        gap: Some(tokens.spacing.s),
                        children: widgets![
                            Icon::svg(material::navigation::check::regular())
                                .size(14.0)
                                .color(tokens.colors.text_secondary),
                            Text::new("All changes saved")
                                .size(tokens.typography.font_size_xs)
                                .line_height(16.0)
                                .color(tokens.colors.text_secondary),
                        ],
                        ..Default::default()
                    }
                    .into()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
