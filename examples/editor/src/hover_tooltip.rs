//! Floating tooltip widget that displays LSP hover information.

use crate::layout::TOOLTIP_MAX_WIDTH;
use crate::model::*;
use crate::palette::{TOOLTIP_BG, TOOLTIP_BORDER, TRANSPARENT};
use fission::prelude::*;
use fission::widgets::Spacer;

pub struct HoverTooltip;

impl From<HoverTooltip> for Widget {
    fn from(_component: HoverTooltip) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        if !view.state().show_hover || view.state().hover_info.is_none() {
            return Spacer {
                height: Some(tokens.spacing.none),
                ..Default::default()
            }
            .into();
        }

        let info = view.state().hover_info.as_ref().unwrap();
        let (hover_x, hover_y) = view.state().hover_position;

        let dismiss = ctx.bind(
            DismissHover,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_hover = false;
                    s.hover_info = None;
                })
            ),
        );

        let tooltip_card = Container::new(
            Text::new(info.as_str())
                .size(tokens.typography.font_size_sm)
                .color(tokens.colors.text_primary),
        )
        .bg(TOOLTIP_BG)
        .border(TOOLTIP_BORDER, 1.0)
        .border_radius(tokens.radii.medium)
        .padding_all(tokens.spacing.s)
        .max_width(TOOLTIP_MAX_WIDTH)
        .into();

        let positioned_tooltip = Positioned {
            left: Some(hover_x),
            top: Some(hover_y),
            child: Some(tooltip_card),
            ..Default::default()
        }
        .into();

        let backdrop = GestureDetector {
            on_tap: Some(dismiss),
            child: Container::new(Spacer::default())
                .bg(TRANSPARENT)
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        }
        .into();

        let overlay = Container::new(ZStack {
            children: vec![
                Positioned {
                    left: Some(tokens.spacing.none),
                    right: Some(tokens.spacing.none),
                    top: Some(tokens.spacing.none),
                    bottom: Some(tokens.spacing.none),
                    child: Some(backdrop),
                    ..Default::default()
                }
                .into(),
                positioned_tooltip,
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
        .into();

        let portal_root = Positioned {
            left: Some(tokens.spacing.none),
            right: Some(tokens.spacing.none),
            top: Some(tokens.spacing.none),
            bottom: Some(tokens.spacing.none),
            child: Some(overlay),
            ..Default::default()
        }
        .into();

        ctx.register_portal_with_layer(
            PortalLayer::Flyout,
            Some(WidgetId::explicit("hover_tooltip")),
            portal_root,
        );

        Spacer {
            height: Some(tokens.spacing.none),
            ..Default::default()
        }
        .into()
    }
}
