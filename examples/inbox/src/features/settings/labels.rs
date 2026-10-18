use crate::model::settings::{label_dropped, set_drag_in_progress};
use crate::model::{InboxState, LabelDropped, SetDragInProgress};
use fission::prelude::*;

/// Labels the reader can drag onto the pin target.
const LABELS: [&str; 5] = ["Work", "Personal", "Travel", "Receipts", "Updates"];

fission_component! {
/// Drag a label onto the pin target to pin it.
    pub(super) LabelSettings<InboxState> |_settings, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let drag_started = cx.bind(SetDragInProgress(true), reduce_with!(set_drag_in_progress));
        let drag_ended = cx.bind(SetDragInProgress(false), reduce_with!(set_drag_in_progress));
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
                            Draggable(
                                key: label,
                                id: Some(WidgetId::explicit(&format!(
                                    "settings.label.{}",
                                    label.to_lowercase()
                                ))),
                                payload: label.as_bytes().to_vec(),
                                on_drag_start: drag_started.clone(),
                                on_drag_end: drag_ended.clone(),
                                child: Tag(label: label),
                            )
                        },
                    ],
                ),
                DragTarget(
                    on_drop: cx.bind(LabelDropped("Pinned".into()), reduce_with!(label_dropped)),
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
