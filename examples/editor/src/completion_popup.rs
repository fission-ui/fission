//! Auto-complete popup widget that displays LSP completion suggestions.

use crate::completion_popup_item::CompletionPopupItem;
use crate::flyout_overlay::FlyoutOverlay;
use crate::layout::{
    COMPLETION_POPUP_HEIGHT_FRACTION, COMPLETION_POPUP_MAX_HEIGHT, COMPLETION_POPUP_MAX_WIDTH,
    COMPLETION_POPUP_MIN_HEIGHT, COMPLETION_POPUP_MIN_WIDTH, OVERLAY_EDGE_GUTTER,
    OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::{on_dismiss_completions, DismissCompletions, EditorState};
use crate::palette::EditorPalette;
use fission::core::ui::{Container, Scroll, Widget};
use fission::core::{reduce_with, FlexDirection, PortalLayer, WidgetId};
use fission::widgets::{Spacer, VStack};

/// Completion suggestions shown just below the caret and kept inside the
/// window. A tap anywhere else closes them.
pub struct CompletionPopup;

impl From<CompletionPopup> for Widget {
    fn from(_component: CompletionPopup) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let state = view.state();
        if state.show_completions && !state.completions.is_empty() {
            let palette = EditorPalette::from_theme(&view.env().theme);
            let tokens = &view.env().theme.tokens;
            let viewport = view.viewport_size();
            let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
                .clamp(COMPLETION_POPUP_MIN_WIDTH, COMPLETION_POPUP_MAX_WIDTH);
            let height = (viewport.height * COMPLETION_POPUP_HEIGHT_FRACTION)
                .clamp(COMPLETION_POPUP_MIN_HEIGHT, COMPLETION_POPUP_MAX_HEIGHT);
            // The hover position stands in for the caret's position on screen.
            let (x, y) = state.hover_position;
            let top = (y + tokens.spacing.l)
                .min((viewport.height - height - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));
            let left = x.min((viewport.width - width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));

            let overlay = FlyoutOverlay {
                on_dismiss: ctx.bind(DismissCompletions, reduce_with!(on_dismiss_completions)),
                backdrop: palette.transparent,
                left,
                top,
                child: CompletionList { width, height }.into(),
            };
            ctx.register_portal_with_layer(
                PortalLayer::Flyout,
                Some(WidgetId::explicit("completion_popup")),
                overlay.into(),
            );
        }

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}

/// The scrolling list of suggestions, with the selected one highlighted.
struct CompletionList {
    width: f32,
    height: f32,
}

impl From<CompletionList> for Widget {
    fn from(list: CompletionList) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let items = state
            .completions
            .iter()
            .enumerate()
            .map(|(index, completion)| {
                CompletionPopupItem {
                    index,
                    label: completion.label.clone(),
                    detail: completion.detail.clone(),
                    kind: completion.kind.clone(),
                    selected: index == state.selected_completion,
                }
                .into()
            })
            .collect();

        Container::new(Scroll {
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
        .max_height(list.height)
        .width(list.width)
        .into()
    }
}
