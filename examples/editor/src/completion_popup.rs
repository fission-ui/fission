//! Auto-complete popup widget that displays LSP completion suggestions.

use crate::completion_popup_item::CompletionPopupItem;
use crate::layout::{
    COMPLETION_POPUP_HEIGHT_FRACTION, COMPLETION_POPUP_MAX_HEIGHT, COMPLETION_POPUP_MAX_WIDTH,
    COMPLETION_POPUP_MIN_HEIGHT, COMPLETION_POPUP_MIN_WIDTH, OVERLAY_EDGE_GUTTER,
    OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::*;
use crate::palette::EditorPalette;
use fission::core::ui::{Container, GestureDetector, Positioned, Scroll, Widget, ZStack};
use fission::core::{reduce_with, FlexDirection, PortalLayer, WidgetId};
use fission::widgets::{Spacer, VStack};

pub struct CompletionPopup;

impl From<CompletionPopup> for Widget {
    fn from(_component: CompletionPopup) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        if !view.state().show_completions || view.state().completions.is_empty() {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let dismiss = ctx.bind(DismissCompletions, reduce_with!(on_dismiss_completions));

        let viewport = view.viewport_size();
        let popup_width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(COMPLETION_POPUP_MIN_WIDTH, COMPLETION_POPUP_MAX_WIDTH);
        let popup_height = (viewport.height * COMPLETION_POPUP_HEIGHT_FRACTION)
            .clamp(COMPLETION_POPUP_MIN_HEIGHT, COMPLETION_POPUP_MAX_HEIGHT);

        let selected_idx = view.state().selected_completion;

        let mut items = Vec::new();
        for (i, completion) in view.state().completions.iter().enumerate() {
            items.push(
                CompletionPopupItem {
                    index: i,
                    label: completion.label.clone(),
                    detail: completion.detail.clone(),
                    kind: completion.kind.clone(),
                    selected: i == selected_idx,
                }
                .into(),
            );
        }

        // Compute position near cursor. Use the hover_position as a rough proxy
        // for cursor screen location; the editor surface can set this more precisely.
        let (popup_x, popup_y) = view.state().hover_position;
        // Offset slightly below the cursor line
        let popup_y = (popup_y + tokens.spacing.l)
            .min((viewport.height - popup_height - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));
        let popup_x = popup_x
            .min((viewport.width - popup_width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));

        let list = Container::new(Scroll {
            direction: FlexDirection::Column,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.none),
                    children: items,
                }
                .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .bg(palette.flyout_bg)
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
        .max_height(popup_height)
        .width(popup_width)
        .into();

        let positioned_popup = Positioned {
            left: Some(popup_x),
            top: Some(popup_y),
            child: Some(list),
            ..Default::default()
        }
        .into();

        // Transparent backdrop to dismiss on tap outside
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Container::new(Spacer::default())
                .bg(palette.transparent)
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        }
        .into();

        let overlay = Container::new(ZStack {
            children: vec![
                Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop),
                    ..Default::default()
                }
                .into(),
                positioned_popup,
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
        .into();

        let portal_root = Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(overlay),
            ..Default::default()
        }
        .into();

        ctx.register_portal_with_layer(
            PortalLayer::Flyout,
            Some(WidgetId::explicit("completion_popup")),
            portal_root,
        );

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}
