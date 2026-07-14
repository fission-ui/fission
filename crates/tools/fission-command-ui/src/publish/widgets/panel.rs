use super::*;

#[derive(Clone)]
pub(super) struct NumberedPanel {
    pub(super) number: usize,
    pub(super) title: String,
    pub(super) subtitle: String,
    pub(super) width: f32,
    pub(super) height: Option<f32>,
    pub(super) children: Vec<Widget>,
    pub(super) tone: PanelTone,
}

#[derive(Clone, Copy)]
pub(super) enum PanelTone {
    Normal,
    Warning,
    Danger,
    Success,
}

impl From<NumberedPanel> for Widget {
    fn from(panel: NumberedPanel) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let accent = match panel.tone {
            PanelTone::Normal => palette.hairline,
            PanelTone::Warning => palette.warning,
            PanelTone::Danger => palette.error,
            PanelTone::Success => palette.success,
        };
        let number_bg = match panel.tone {
            PanelTone::Normal => palette.panel_soft,
            PanelTone::Warning => palette.warning,
            PanelTone::Danger => palette.error,
            PanelTone::Success => palette.success,
        };
        let number_color = if matches!(panel.tone, PanelTone::Normal) {
            palette.text
        } else {
            palette.accent_text
        };
        let mut children = widgets![Row {
            gap: Some(if layout.terminal { 1.0 } else { 10.0 }),
            align_items: AlignItems::Center,
            children: widgets![
                Container::new(
                    Text::new(panel.number.to_string())
                        .size(if layout.terminal { 13.0 } else { 16.0 })
                        .color(number_color)
                )
                .width(if layout.terminal { 3.0 } else { 30.0 })
                .height(if layout.terminal { 1.0 } else { 30.0 })
                .bg(number_bg)
                .border_radius(999.0),
                Column {
                    gap: Some(if layout.terminal { 0.0 } else { 3.0 }),
                    children: widgets![
                        Text::new(panel.title)
                            .size(if layout.terminal { 13.0 } else { 17.0 })
                            .color(palette.text),
                        Text::new(panel.subtitle)
                            .size(if layout.terminal { 11.0 } else { 12.5 })
                            .color(palette.muted),
                    ],
                    ..Default::default()
                }
                .flex_grow(1.0),
            ],
            ..Default::default()
        }];
        if !panel.children.is_empty() {
            children.push(
                DividerLine {
                    color: palette.hairline,
                }
                .into(),
            );
            children.extend(panel.children);
        }
        let mut container = Container::new(Column {
            gap: Some(if layout.terminal { 1.0 } else { 10.0 }),
            children,
            ..Default::default()
        })
        .width(panel.width)
        .padding([
            layout.card_padding,
            layout.card_padding,
            layout.card_padding,
            layout.card_padding,
        ])
        .bg(palette.panel)
        .border(accent, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius);
        if let Some(height) = panel.height {
            container = container.height(height);
        }
        container.into()
    }
}

#[derive(Clone)]
pub(super) struct DividerLine {
    pub(super) color: Color,
}

impl From<DividerLine> for Widget {
    fn from(line: DividerLine) -> Widget {
        Container::new(Spacer::default())
            .height(1.0)
            .bg(line.color)
            .into()
    }
}
