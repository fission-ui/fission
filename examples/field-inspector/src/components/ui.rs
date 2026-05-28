use crate::model::CapabilityState;
use fission::prelude::*;

pub fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub fn is_compact<S: AppState>(view: &View<S>) -> bool {
    view.viewport_size().width < 760.0
}

pub fn page_padding<S: AppState>(view: &View<S>) -> f32 {
    if view.viewport_size().width < 520.0 {
        10.0
    } else if is_compact(view) {
        16.0
    } else {
        24.0
    }
}

pub fn usable_width<S: AppState>(view: &View<S>, reserved: f32) -> f32 {
    (view.viewport_size().width - reserved).max(240.0)
}

pub fn responsive_grid<S: AppState>(
    view: &View<S>,
    children: Vec<Node>,
    wide_columns: usize,
) -> Node {
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
                    .into_node()
            })
            .collect(),
        ..Default::default()
    }
    .into_node()
}

pub fn muted_text<S: AppState>(view: &View<S>, text: impl Into<String>) -> Node {
    Text::new(text.into())
        .size(13.0)
        .line_height(19.0)
        .color(view.env.theme.tokens.colors.text_secondary)
        .into_node()
}

pub fn body_text<S: AppState>(view: &View<S>, text: impl Into<String>) -> Node {
    Text::new(text.into())
        .size(14.0)
        .line_height(21.0)
        .color(view.env.theme.tokens.colors.text_primary)
        .into_node()
}

pub fn title_text<S: AppState>(view: &View<S>, text: impl Into<String>, size: f32) -> Node {
    Text::new(text.into())
        .size(size)
        .line_height(size + 8.0)
        .weight(800)
        .color(view.env.theme.tokens.colors.text_primary)
        .into_node()
}

pub fn panel_card<S: AppState>(view: &View<S>, child: Node) -> Node {
    let tokens = &view.env.theme.tokens;
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
        .into_node()
}

pub fn soft_panel<S: AppState>(view: &View<S>, child: Node) -> Node {
    let tokens = &view.env.theme.tokens;
    let compact = is_compact(view);
    Container::new(child)
        .bg(tokens.colors.background.with_alpha(170))
        .border(tokens.colors.border.with_alpha(120), 1.0)
        .border_radius(if compact { 14.0 } else { 18.0 })
        .padding_all(if compact { 10.0 } else { 14.0 })
        .into_node()
}

pub fn action_button(
    label: impl Into<String>,
    action: ActionEnvelope,
    variant: ButtonVariant,
) -> Node {
    Button {
        child: Some(Box::new(Text::new(label.into()).weight(700).into_node())),
        on_press: Some(action),
        variant,
        min_width: Some(132.0),
        ..Default::default()
    }
    .into_node()
}

pub fn small_button(
    label: impl Into<String>,
    action: ActionEnvelope,
    variant: ButtonVariant,
) -> Node {
    Button {
        child: Some(Box::new(
            Text::new(label.into()).size(13.0).weight(700).into_node(),
        )),
        on_press: Some(action),
        variant,
        size: ComponentSize::Sm,
        ..Default::default()
    }
    .into_node()
}

pub fn status_pill<S: AppState>(
    view: &View<S>,
    label: impl Into<String>,
    state: CapabilityState,
) -> Node {
    let (bg, fg) = match state {
        CapabilityState::Idle => (
            view.env.theme.tokens.colors.surface,
            view.env.theme.tokens.colors.text_secondary,
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
            .size(12.0)
            .line_height(16.0)
            .weight(800)
            .color(fg)
            .into_node(),
    )
    .bg(bg)
    .border_radius(999.0)
    .padding([10.0, 10.0, 4.0, 4.0])
    .into_node()
}

pub fn metric<S: AppState>(
    view: &View<S>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> Node {
    soft_panel(
        view,
        Column {
            gap: Some(4.0),
            children: vec![
                muted_text(view, label.into()),
                Text::new(value.into())
                    .size(19.0)
                    .line_height(25.0)
                    .weight(800)
                    .color(view.env.theme.tokens.colors.text_primary)
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
}
