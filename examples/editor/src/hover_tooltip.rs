//! Floating tooltip widget that displays LSP hover information.

use crate::layout::TOOLTIP_MAX_WIDTH;
use crate::model::*;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub HoverTooltip<EditorState> |_tooltip, state, cx| {
        let info = state
            .hover_info
            .as_ref()
            .filter(|_| state.show_hover);
        if let Some(info) = info {
            cx.register_portal_with_layer(
                PortalLayer::Flyout,
                Some(WidgetId::explicit("hover_tooltip")),
                HoverOverlay::new(info.clone(), state.hover_position).into(),
            );
        }
    } =>
    Spacer(height: 0.0)
}

fission_component! {
    HoverOverlay<EditorState> {
        info: String,
        position: (f32, f32),
    } |tooltip, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Positioned(
        left: Some(tokens.spacing.none),
        right: Some(tokens.spacing.none),
        top: Some(tokens.spacing.none),
        bottom: Some(tokens.spacing.none),
        child: Container(
            ZStack(
                children: [
                    Positioned(
                        left: Some(tokens.spacing.none),
                        right: Some(tokens.spacing.none),
                        top: Some(tokens.spacing.none),
                        bottom: Some(tokens.spacing.none),
                        child: GestureDetector(
                            on_tap: |_tooltip, state, _cx| {
                                state.show_hover = false;
                                state.hover_info = None;
                            },
                            child: Container(Spacer())
                                .bg(palette.transparent)
                                .flex_grow(1.0),
                        ),
                    ),
                    Positioned(
                        left: Some(tooltip.position.0),
                        top: Some(tooltip.position.1),
                        child: Container(
                            Text(tooltip.info.clone())
                                .size(tokens.typography.font_size_sm)
                                .color(tokens.colors.text_primary),
                        )
                        .bg(palette.tooltip_bg)
                        .border(palette.tooltip_border, 1.0)
                        .border_radius(tokens.radii.medium)
                        .padding_all(tokens.spacing.s)
                        .max_width(TOOLTIP_MAX_WIDTH),
                    ),
                ],
            ),
        ).flex_grow(1.0),
    )
}
