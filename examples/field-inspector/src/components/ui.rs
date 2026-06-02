use crate::model::CapabilityState;
use fission::prelude::*;

pub fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub fn is_compact<S: GlobalState>(view: ViewHandle<S>) -> bool {
    view.viewport_size().width < 760.0
}

pub fn page_padding<S: GlobalState>(view: ViewHandle<S>) -> f32 {
    if view.viewport_size().width < 520.0 {
        10.0
    } else if is_compact(view) {
        16.0
    } else {
        24.0
    }
}

pub fn usable_width<S: GlobalState>(view: ViewHandle<S>, reserved: f32) -> f32 {
    (view.viewport_size().width - reserved).max(240.0)
}

pub fn responsive_grid<S: GlobalState>(
    view: ViewHandle<S>,
    children: Vec<Widget>,
    wide_columns: usize,
) -> Widget {
    let columns = if is_compact(view) {
        1
    } else {
        wide_columns.max(1)
    };
    Grid {
        columns: (0..columns).map(|_| ir_op::GridTrack::Fr(1.0)).collect(),
        column_gap: Some(12.0),
        row_gap: Some(12.0),
        children: children
            .into_iter()
            .enumerate()
            .map(|(index, child)| {
                GridItem::new(child)
                    .cell((index / columns + 1) as i16, (index % columns + 1) as i16)
                    .into()
            })
            .collect(),
        ..Default::default()
    }
    .into()
}

pub fn muted_text<S: GlobalState>(view: ViewHandle<S>, text: impl Into<String>) -> Widget {
    let compact = is_compact(view);
    Text::new(text.into())
        .size(if compact { 12.0 } else { 13.0 })
        .line_height(if compact { 18.0 } else { 19.0 })
        .color(view.env().theme.tokens.colors.text_secondary)
        .into()
}

pub fn body_text<S: GlobalState>(view: ViewHandle<S>, text: impl Into<String>) -> Widget {
    let compact = is_compact(view);
    Text::new(text.into())
        .size(if compact { 13.0 } else { 14.0 })
        .line_height(if compact { 19.0 } else { 21.0 })
        .color(view.env().theme.tokens.colors.text_primary)
        .into()
}

pub fn title_text<S: GlobalState>(
    view: ViewHandle<S>,
    text: impl Into<String>,
    size: f32,
) -> Widget {
    let compact = is_compact(view);
    let size = if compact { size.min(22.0) } else { size };
    let mut title = Text::new(text.into())
        .size(size)
        .line_height(size + if compact { 6.0 } else { 8.0 })
        .weight(800)
        .color(view.env().theme.tokens.colors.text_primary);
    if compact {
        title = title.max_width(usable_width(view, 64.0));
    }
    title.into()
}

pub fn panel_card<S: GlobalState>(view: ViewHandle<S>, child: Widget) -> Widget {
    let tokens = &view.env().theme.tokens;
    let compact = is_compact(view);
    Container::new(child)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border.with_alpha(150), 1.0)
        .border_radius(if compact { 16.0 } else { 22.0 })
        .padding_all(if compact { 12.0 } else { 18.0 })
        .shadow(ir_op::BoxShadow {
            color: Color {
                r: 15,
                g: 23,
                b: 42,
                a: 18,
            },
            blur_radius: 18.0,
            offset: (0.0, 8.0),
        })
        .into()
}

pub fn soft_panel<S: GlobalState>(view: ViewHandle<S>, child: Widget) -> Widget {
    let tokens = &view.env().theme.tokens;
    let compact = is_compact(view);
    Container::new(child)
        .bg(tokens.colors.background.with_alpha(170))
        .border(tokens.colors.border.with_alpha(120), 1.0)
        .border_radius(if compact { 14.0 } else { 18.0 })
        .padding_all(if compact { 10.0 } else { 14.0 })
        .into()
}

pub fn action_button(
    label: impl Into<String>,
    action: ActionEnvelope,
    variant: ButtonVariant,
) -> Widget {
    Button {
        child: Some(Text::new(label.into()).weight(700).into()),
        on_press: Some(action),
        variant,
        min_width: Some(132.0),
        ..Default::default()
    }
    .into()
}

pub fn small_button(
    label: impl Into<String>,
    action: ActionEnvelope,
    variant: ButtonVariant,
) -> Widget {
    Button {
        child: Some(Text::new(label.into()).size(13.0).weight(700).into()),
        on_press: Some(action),
        variant,
        size: ComponentSize::Sm,
        ..Default::default()
    }
    .into()
}

pub fn status_pill<S: GlobalState>(
    view: ViewHandle<S>,
    label: impl Into<String>,
    state: CapabilityState,
) -> Widget {
    let (bg, fg) = match state {
        CapabilityState::Idle => (
            view.env().theme.tokens.colors.surface,
            view.env().theme.tokens.colors.text_secondary,
        ),
        CapabilityState::Pending => (color(254, 243, 199), color(146, 64, 14)),
        CapabilityState::Ready => (color(219, 234, 254), color(29, 78, 216)),
        CapabilityState::Complete => (color(220, 252, 231), color(21, 128, 61)),
        CapabilityState::Unavailable => (color(229, 231, 235), color(75, 85, 99)),
        CapabilityState::Warning => (color(254, 249, 195), color(133, 77, 14)),
        CapabilityState::Error => (color(254, 226, 226), color(185, 28, 28)),
    };
    Container::new(
        Text::new(label.into())
            .size(if is_compact(view) { 11.0 } else { 12.0 })
            .line_height(if is_compact(view) { 15.0 } else { 16.0 })
            .weight(800)
            .wrap(false)
            .color(fg),
    )
    .bg(bg)
    .border_radius(999.0)
    .padding(if is_compact(view) {
        [8.0, 8.0, 4.0, 4.0]
    } else {
        [10.0, 10.0, 4.0, 4.0]
    })
    .into()
}

pub fn metric<S: GlobalState>(
    view: ViewHandle<S>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> Widget {
    soft_panel(
        view,
        Column {
            gap: Some(4.0),
            children: vec![
                muted_text(view, label.into()),
                Text::new(value.into())
                    .size(if is_compact(view) { 17.0 } else { 19.0 })
                    .line_height(if is_compact(view) { 23.0 } else { 25.0 })
                    .weight(800)
                    .color(view.env().theme.tokens.colors.text_primary)
                    .into(),
            ],
            ..Default::default()
        }
        .into(),
    )
}
