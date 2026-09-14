use crate::model::settings::{label_dropped, set_drag_in_progress};
use crate::model::{InboxState, LabelDropped, SetDragInProgress};
use fission::core::op::FlexDirection;
use fission::core::ui::{Container, Text, TextContent, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::widgets::{Badge, DragTarget, Draggable, Tag, VStack, Wrap};

/// Labels the reader can drag onto the pin target.
const LABELS: [&str; 5] = ["Work", "Personal", "Travel", "Receipts", "Updates"];

/// Drag a label onto the pin target to pin it.
pub(super) struct LabelSettings;

impl From<LabelSettings> for Widget {
    fn from(_: LabelSettings) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let drag_started = ctx.bind(SetDragInProgress(true), reduce_with!(set_drag_in_progress));
        let drag_ended = ctx.bind(SetDragInProgress(false), reduce_with!(set_drag_in_progress));

        let pinned: Widget = match &state.last_drag_label {
            Some(label) => Badge {
                text: format!("{}: {label}", view.tr("settings.labels.pinned")),
                ..Default::default()
            }
            .into(),
            None => Text::new(TextContent::Key("settings.labels.helper".into()))
                .size(tokens.typography.font_size_xs)
                .color(tokens.colors.text_secondary)
                .into(),
        };

        VStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(TextContent::Key("settings.labels.title".into()))
                    .size(tokens.typography.font_size_xs)
                    .into(),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.xs),
                    run_spacing: None,
                    children: LABELS
                        .iter()
                        .map(|label| {
                            Draggable {
                                id: Some(WidgetId::explicit(&format!(
                                    "settings.label.{}",
                                    label.to_lowercase()
                                ))),
                                semantics_identifier: None,
                                payload: label.as_bytes().to_vec(),
                                on_drag_start: Some(drag_started.clone()),
                                on_drag_end: Some(drag_ended.clone()),
                                child: Tag {
                                    label: (*label).into(),
                                    on_close: None,
                                    on_press: None,
                                    selected: false,
                                }
                                .into(),
                                preview: None,
                                preview_options: Default::default(),
                            }
                            .into()
                        })
                        .collect(),
                }
                .into(),
                DragTarget {
                    id: None,
                    semantics_identifier: None,
                    on_drop: Some(
                        ctx.bind(LabelDropped("Pinned".into()), reduce_with!(label_dropped)),
                    ),
                    child: Container::new(
                        Text::new(TextContent::Key("settings.labels.drop_target".into()))
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
                    .border_radius(tokens.radii.medium)
                    .into(),
                    hover_child: None,
                }
                .into(),
                pinned,
            ],
        }
        .into()
    }
}
