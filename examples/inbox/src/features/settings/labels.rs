use crate::model::InboxState;
use fission::prelude::*;

/// Labels the reader can drag onto the pin target.
const LABELS: [&str; 5] = ["Work", "Personal", "Travel", "Receipts", "Updates"];

fission_component! {
/// Drag a label onto the pin target to pin it.
    pub(super) LabelSettings<InboxState> |_settings, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(TextContent::Key("settings.labels.title".into()))
                    .size(tokens.typography.font_size_xs),
                Wrap(
                    direction: FlexDirection::Row,
                    spacing: tokens.spacing.xs,
                    children: [
                        for label in LABELS {
                            DraggableLabel(label, key: label)
                        },
                    ],
                ),
                DragTarget(
                    on_drop: |_settings, state, cx| {
                        if let Some(label) = cx
                            .input
                            .as_internal_drop()
                            .and_then(|payload| String::from_utf8(payload.to_vec()).ok())
                        {
                            pin_label(state, label);
                        }
                    },
                    child: Container(
                        Text(TextContent::Key("settings.labels.drop_target".into()))
                            .size(tokens.typography.font_size_xs),
                    )
                    .padding_all(tokens.spacing.s)
                    .bg(if state.drag_in_progress {
                        tokens.colors.primary_subtle
                    } else {
                        tokens.colors.background.with_alpha(0)
                    })
                    .border(tokens.colors.border, 1.0)
                    .border_dash(vec![tokens.spacing.xs, tokens.spacing.xs])
                    .border_radius(tokens.radii.medium),
                ),
                match &state.last_drag_label {
                    Some(label) => Badge(
                        text: format!("{}: {label}", cx.tr("settings.labels.pinned")),
                    ),
                    None => Text(TextContent::Key("settings.labels.helper".into()))
                        .size(tokens.typography.font_size_xs)
                        .color(tokens.colors.text_secondary),
                },
            ],
        )
}

pub(crate) fn pin_label(state: &mut InboxState, label: String) {
    state.last_drag_label = Some(label);
    state.drag_in_progress = false;
}

fission_component! {
    DraggableLabel<InboxState> { label: &'static str } |item, _state, _cx| =>
        Draggable(
            id: WidgetId::explicit(&format!(
                "settings.label.{}",
                item.label.to_lowercase()
            )),
            payload: item.label.as_bytes().to_vec(),
            on_drag_start: |_item, state, _cx| state.drag_in_progress = true,
            on_drag_end: |_item, state, _cx| state.drag_in_progress = false,
            child: Tag(label: item.label),
        )
}
