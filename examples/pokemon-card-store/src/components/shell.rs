use crate::app::StoreState;
use fission::prelude::*;

#[derive(Clone)]
pub struct StoreShell {
    pub child: Widget,
}

impl From<StoreShell> for Widget {
    fn from(component: StoreShell) -> Self {
        let (_ctx, view) = fission::build::current::<StoreState>();
        let viewport = view.viewport_size();
        Container::new(Column {
            gap: Some(26.0),
            children: vec![nav(view), component.child.clone(), footer(view)],
            ..Default::default()
        })
        .min_height(viewport.height.max(900.0))
        .padding([36.0, 36.0, 24.0, 36.0])
        .bg(color(12, 18, 32))
        .into()
    }
}
fn nav(view: ViewHandle<StoreState>) -> Widget {
    let _ = view;
    Row {
        gap: Some(18.0),
        children: vec![
            Text::new("Fission Card Market")
                .size(22.0)
                .line_height(28.0)
                .weight(800)
                .color(color(248, 250, 252))
                .semantics_identifier("site-route:/")
                .into(),
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
            nav_link("Catalogue", "/"),
            pill("Session cart", color(34, 197, 94)),
            pill("Worker filters", color(96, 165, 250)),
            pill("Cart island", color(244, 114, 182)),
        ],
        align_items: ir_op::AlignItems::Center,
        ..Default::default()
    }
    .into()
}

fn nav_link(label: &str, href: &str) -> Widget {
    Text::new(label)
        .size(14.0)
        .line_height(20.0)
        .weight(800)
        .color(color(191, 219, 254))
        .semantics_identifier(format!("site-route:{href}"))
        .into()
}

fn footer(_view: ViewHandle<StoreState>) -> Widget {
    Text::new("Demo storefront: server rendering, session state, route-local workers, and focused WASM islands.")
    .size(13.0)
    .line_height(20.0)
    .color(color(148, 163, 184))
    .into()
}

fn pill(label: &str, accent: Color) -> Widget {
    Container::new(
        Text::new(label)
            .size(12.0)
            .line_height(16.0)
            .weight(700)
            .color(color(226, 232, 240)),
    )
    .padding([10.0, 10.0, 6.0, 6.0])
    .border(accent.with_alpha(160), 1.0)
    .border_radius(999.0)
    .bg(accent.with_alpha(34))
    .into()
}

fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
