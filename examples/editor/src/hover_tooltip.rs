//! Floating tooltip widget that displays LSP hover information.

use crate::model::*;
use fission::core::op::Color;
use fission::core::ui::{Container, GestureDetector, Widget, Positioned, Text, ZStack};
use fission::core::{BuildCtxHandle, reduce_with, PortalLayer, ViewHandle, WidgetId};
use fission::widgets::Spacer;

pub struct HoverTooltip;

impl From<HoverTooltip> for Widget {
    fn from(component: HoverTooltip) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        if !view.state().show_hover || view.state().hover_info.is_none() {
            return Spacer { height: Some(0.0), ..Default::default() }.into();
        }

        let info = view.state().hover_info.as_ref().unwrap();
        let (hover_x, hover_y) = view.state().hover_position;

        let dismiss = ctx.bind(
            DismissHover,
            reduce_with!((|s: &mut EditorState, _, _| {
                s.show_hover = false;
                s.hover_info = None;
            })),
        );

        let bg = Color { r: 45, g: 45, b: 46, a: 255 };
        let border_color = Color { r: 80, g: 80, b: 80, a: 255 };
        let text_color = Color { r: 220, g: 220, b: 220, a: 255 };

        // Tooltip card with hover content
        let tooltip_card = Container::new(
            Text::new(info.as_str())
                .size(12.0)
                .color(text_color)
        )
        .bg(bg)
        .border(border_color, 1.0)
        .border_radius(4.0)
        .padding_all(8.0)
        .max_width(400.0)
        .into();

        // Position the tooltip at the hover location
        let positioned_tooltip = Positioned {
            left: Some(hover_x),
            top: Some(hover_y),
            child: Some(tooltip_card),
            ..Default::default()
        }
        .into();

        // Transparent backdrop to dismiss on tap elsewhere
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child:
                Container::new(Spacer::default())
                    .bg(Color { r: 0, g: 0, b: 0, a: 0 })
                    .flex_grow(1.0)
                    .into(),
            ..Default::default()
        }
        .into();

        let overlay = Container::new(
            ZStack {
                children: vec![
                    // Full-screen transparent backdrop for dismissal
                    Positioned {
                        left: Some(0.0),
                        right: Some(0.0),
                        top: Some(0.0),
                        bottom: Some(0.0),
                        child: Some(backdrop),
                        ..Default::default()
                    }
                    .into(),
                    // The tooltip itself
                    positioned_tooltip,
                ],
                ..Default::default()
            }
        )
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
            Some(WidgetId::explicit("hover_tooltip")),
            portal_root,
        );

        Spacer { height: Some(0.0), ..Default::default() }.into()

    }
}
