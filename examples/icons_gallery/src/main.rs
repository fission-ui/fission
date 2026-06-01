use fission::core::op::Color;
use fission::core::AppState;
use fission::op::AlignItems;
use fission::prelude::DesktopApp;
use fission::widgets::{
    BuildCtx, Column, Container, Icon, LazyColumn, Node, Row, Text, View, Widget,
};
use lazy_static::lazy_static;
use std::sync::Arc;

fn build_icon_rows() -> Vec<Node> {
    let all = fission::icons::material::all_icons();
    let mut items = Vec::with_capacity(all.len());

    for (idx, (cat, name, variant, func)) in all.into_iter().enumerate() {
        let label = format!("{}/{}/{}", cat, name, variant);
        let row = Row {
            gap: Some(12.0),
            align_items: AlignItems::Center,
            children: vec![
                Icon::svg(func()).size(24.0).into_node(),
                Text::new(label)
                    .size(12.0)
                    .color(Color {
                        r: 80,
                        g: 80,
                        b: 80,
                        a: 255,
                    })
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node();

        let item = Container::new(row)
            .height(56.0)
            .padding_all(8.0)
            .bg(if idx % 2 == 0 {
                Color::WHITE
            } else {
                Color {
                    r: 248,
                    g: 248,
                    b: 248,
                    a: 255,
                }
            })
            .border(
                Color {
                    r: 230,
                    g: 230,
                    b: 230,
                    a: 255,
                },
                1.0,
            )
            .into_node();

        items.push(item);
    }

    items
}

lazy_static! {
    static ref ICON_ROWS: Arc<Vec<Node>> = Arc::new(build_icon_rows());
}

#[derive(Default, Clone, Debug)]
struct State;

impl AppState for State {}

struct IconsApp;

impl Widget<State> for IconsApp {
    fn build(
        &self,
        _ctx: &mut BuildCtx<State>,
        _view: &View<State>,
    ) -> impl fission::IntoWidget<State> {
        fission::AnyWidget::from_node({
            let title = Text::new("Material Icons Gallery").size(32.0);

            let total = ICON_ROWS.len();
            let item_height = 56.0;

            let content = LazyColumn {
                id: None,
                children: ICON_ROWS.clone(),
                item_height,
            }
            .into_node();

            Container::new(
                Column {
                    gap: Some(24.0),
                    flex_grow: 1.0,
                    children: vec![
                        title.into_node(),
                        Text::new(format!("{} icon variants", total))
                            .size(14.0)
                            .into_node(),
                        content,
                    ],
                    ..Default::default()
                }
                .into_node(),
            )
            .padding_all(24.0)
            .bg(Color {
                r: 245,
                g: 245,
                b: 245,
                a: 255,
            })
            .flex_grow(1.0)
            .into_node()
        })
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(IconsApp);
    app.run()
}
