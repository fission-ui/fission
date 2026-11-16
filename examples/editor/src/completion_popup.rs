//! Auto-complete popup widget that displays LSP completion suggestions.

use crate::completion_popup_item::CompletionPopupItem;
use crate::flyout_overlay::{FlyoutDismiss, FlyoutOverlay};
use crate::layout::{
    COMPLETION_POPUP_HEIGHT_FRACTION, COMPLETION_POPUP_MAX_HEIGHT, COMPLETION_POPUP_MAX_WIDTH,
    COMPLETION_POPUP_MIN_HEIGHT, COMPLETION_POPUP_MIN_WIDTH, OVERLAY_EDGE_GUTTER,
    OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
/// Completion suggestions shown just below the caret and kept inside the
/// window. A tap anywhere else closes them.
    pub CompletionPopup<EditorState> |_popup, state, cx| {
        if state.show_completions && !state.completions.is_empty() {
            let palette = EditorPalette::from_theme(&cx.view.env().theme);
            let tokens = &cx.view.env().theme.tokens;
            let viewport = cx.view.viewport_size();
            let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
                .clamp(COMPLETION_POPUP_MIN_WIDTH, COMPLETION_POPUP_MAX_WIDTH);
            let height = (viewport.height * COMPLETION_POPUP_HEIGHT_FRACTION)
                .clamp(COMPLETION_POPUP_MIN_HEIGHT, COMPLETION_POPUP_MAX_HEIGHT);
            // The hover position stands in for the caret's position on screen.
            let (x, y) = state.hover_position;
            let top = (y + tokens.spacing.l)
                .min((viewport.height - height - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));
            let left = x.min((viewport.width - width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));

            cx.register_portal_with_layer(
                PortalLayer::Flyout,
                Some(WidgetId::explicit("completion_popup")),
                FlyoutOverlay::new(
                    FlyoutDismiss::Completions,
                    palette.transparent,
                    left,
                    top,
                    CompletionList::new(width, height).into(),
                ).into(),
            );
        }
    } =>
    Spacer(height: 0.0)
}

fission_component! {
/// The scrolling list of suggestions, with the selected one highlighted.
    CompletionList<EditorState> { width: f32, height: f32 } |list, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Scroll(
            direction: FlexDirection::Column,
            child: VStack(
                spacing: tokens.spacing.none,
                children: [
                    for (index, completion) in state.completions.iter().enumerate() {
                        CompletionPopupItem(
                            index,
                            completion.label.clone(),
                            completion.detail.clone(),
                            completion.kind.clone(),
                            index == state.selected_completion,
                            key: format!("{index}:{}", completion.label),
                        )
                    },
                ],
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ),
        )
        .bg(palette.flyout_bg)
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
        .max_height(list.height)
        .width(list.width)
}
