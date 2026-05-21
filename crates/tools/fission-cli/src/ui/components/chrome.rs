use super::{ActionButton, ButtonTone, OutputPanel};
use crate::ui::actions::{navigate, toggle_theme, Navigate, ToggleTheme};
use crate::ui::density::UiDensity;
use crate::ui::routes::UiRoute;
use crate::ui::state::UiState;
use crate::ui::theme::UiPalette;
use fission::ir::op::{AlignItems, JustifyContent};
use fission::ir::NodeId;
use fission::prelude::*;

const NAV_SCROLL_NODE_ID: &str = "cli_ui_nav_scroll";

#[derive(Clone)]
pub(crate) struct AppShell {
    pub(crate) content: Node,
}

impl Widget<UiState> for AppShell {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let viewport = view.env.viewport_size;
        let density = UiDensity::new(view.state.compact_mode);
        let width = viewport.width.max(96.0);
        let height = viewport.height.max(28.0);
        let metrics = density.shell_metrics(height);
        let outer_padding = density.outer_padding();
        let sidebar_w = density.sidebar_width();
        let body_gap = density.body_gap();
        let content_w =
            (width - sidebar_w - outer_padding[0] - outer_padding[1] - body_gap).max(48.0);

        Container::new(
            Column {
                gap: Some(density.shell_gap()),
                children: vec![
                    AppHeader.build(ctx, view),
                    Row {
                        gap: Some(body_gap),
                        align_items: AlignItems::Stretch,
                        children: vec![
                            Sidebar {
                                width: sidebar_w,
                                height: metrics.body_h,
                            }
                            .build(ctx, view),
                            Container::new(self.content.clone())
                                .width(content_w)
                                .height(metrics.body_h)
                                .padding(density.content_padding())
                                .bg(palette.surface)
                                .border(palette.border, 1.0)
                                .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    OutputPanel {
                        width: width - outer_padding[0] - outer_padding[1],
                        height: metrics.footer_h,
                    }
                    .build(ctx, view),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .width(width)
        .height(height)
        .padding(outer_padding)
        .bg(palette.background)
        .into_node()
    }
}

#[derive(Clone)]
pub(crate) struct AppHeader;

impl Widget<UiState> for AppHeader {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let density = UiDensity::new(view.state.compact_mode);
        let toggle = with_reducer!(ctx, ToggleTheme, toggle_theme);
        Container::new(
            Row {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                children: vec![
                    Column {
                        gap: Some(0.0),
                        children: vec![
                            Text::new("Fission CLI")
                                .color(palette.accent_text)
                                .into_node(),
                            Text::new(format!(
                                "{}  -  {}",
                                view.state.project_name, view.state.project_status
                            ))
                            .color(palette.accent_text)
                            .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    Row {
                        gap: Some(1.0),
                        children: vec![
                            Text::new(format!("Theme: {}", view.state.theme_mode.label()))
                                .color(palette.accent_text)
                                .into_node(),
                            ActionButton::new("Switch theme", toggle)
                                .tone(ButtonTone::Neutral)
                                .width(16.0)
                                .build(ctx, view),
                            Text::new("q / Esc / Ctrl-C asks to exit")
                                .color(palette.accent_text)
                                .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .height(density.header_height())
        .padding(density.content_padding())
        .bg(palette.accent)
        .border(palette.accent, 1.0)
        .into_node()
    }
}

#[derive(Clone)]
pub(crate) struct Sidebar {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl Widget<UiState> for Sidebar {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let density = UiDensity::new(view.state.compact_mode);
        let padding = density.sidebar_padding();
        let scroll_id = NodeId::explicit(NAV_SCROLL_NODE_ID);
        let scroll_height = (self.height - padding[2] - padding[3]).max(1.0);
        let route_height = density.nav_route_height();
        let offset = view.runtime.scroll.get_offset(scroll_id).max(0.0);
        let start_route = ((offset / route_height).floor() as usize)
            .min(UiRoute::SIDEBAR.len().saturating_sub(1));
        let visible_routes = ((scroll_height - 1.0) / route_height).floor().max(1.0) as usize;
        let mut children = vec![
            Text::new("Workflows").color(palette.accent).into_node(),
            Spacer {
                height: Some(start_route as f32 * route_height),
                ..Default::default()
            }
            .into_node(),
        ];
        for route in UiRoute::SIDEBAR
            .iter()
            .copied()
            .skip(start_route)
            .take(visible_routes)
        {
            children.push(route_button(route, ctx, view, self.width - 4.0));
        }
        let bottom_routes = UiRoute::SIDEBAR
            .len()
            .saturating_sub(start_route.saturating_add(visible_routes));
        children.push(
            Spacer {
                height: Some(bottom_routes as f32 * route_height),
                ..Default::default()
            }
            .into_node(),
        );
        Container::new(
            Scroll {
                id: Some(scroll_id),
                direction: FlexDirection::Column,
                width: Some(self.width - 2.0),
                height: Some(scroll_height),
                show_scrollbar: true,
                child: Some(Box::new(
                    Column {
                        gap: Some(density.nav_gap()),
                        children,
                        ..Default::default()
                    }
                    .into_node(),
                )),
                ..Default::default()
            }
            .into_node(),
        )
        .width(self.width)
        .height(self.height)
        .padding(padding)
        .bg(palette.raised)
        .border(palette.border, 1.0)
        .into_node()
    }
}

fn route_button(
    route: UiRoute,
    ctx: &mut BuildCtx<UiState>,
    view: &View<UiState>,
    width: f32,
) -> Node {
    let action = with_reducer!(ctx, Navigate(route), navigate);
    let tone = if view.state.route == route {
        ButtonTone::Primary
    } else {
        ButtonTone::Neutral
    };
    ActionButton::new(route.title(), action)
        .tone(tone)
        .width(width)
        .build(ctx, view)
}
