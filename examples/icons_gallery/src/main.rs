use fission_widgets::{
    Container, Icon, Row, Scroll, Text, VStack, BuildCtx, View, Node, Widget, Tooltip,
};
use fission_core::op::{Color, BoxShadow};
use fission_core::{AppState, WidgetNodeId};
use fission_shell_desktop::DesktopApp;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug)]
struct State {
    filter: String,
}

impl AppState for State {}

struct IconsApp;

impl Widget<State> for IconsApp {
    fn build(&self, ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
        let title = Text::new("Material Icons Gallery")
            .size(32.0);

        // Group icons by (Category, Name)
        let all = fission_icons::material::all_icons();
        let mut grouped: HashMap<(String, String), HashMap<String, fn() -> &'static str>> = HashMap::new();
        
        for (cat, name, variant, func) in all {
            grouped.entry((cat.to_string(), name.to_string()))
                .or_default()
                .insert(variant.to_string(), func);
        }

        let mut keys: Vec<_> = grouped.keys().cloned().collect();
        keys.sort();

        let mut grid_items = Vec::new();

        // Limit to first 200 for performance
        for (idx, (cat, name)) in keys.into_iter().take(200).enumerate() {
            let variants = &grouped[&(cat.clone(), name.clone())];
            
            if let (Some(regular), Some(outlined)) = (variants.get("regular"), variants.get("outlined")) {
                let mut regular_node = Icon::svg(regular()).size(32.0).into_node();
                
                // Add tooltip to the very first icon to verify interactions
                if idx == 0 {
                    regular_node = Tooltip {
                        id: WidgetNodeId::explicit(&format!("regular_{}", name)),
                        text: format!("{} (Regular)", name),
                        child: Box::new(regular_node),
                        is_visible: false,
                    }.build(ctx, _view);
                }

                let card = Container::new(
                    VStack {
                        spacing: Some(8.0),
                        children: vec![
                            Text::new(format!("{} / {}", cat, name)).size(12.0).color(Color { r: 100, g: 100, b: 100, a: 255 }).into_node(),
                            Row {
                                gap: Some(16.0),
                                children: vec![
                                    VStack {
                                        spacing: Some(4.0),
                                        children: vec![
                                            regular_node,
                                            Text::new("Regular").size(10.0).color(Color::BLACK).into_node(),
                                        ]
                                    }.into_node(),
                                    VStack {
                                        spacing: Some(4.0),
                                        children: vec![
                                            Icon::svg(outlined()).size(32.0).into_node(),
                                            Text::new("Outlined").size(10.0).color(Color::BLACK).into_node(),
                                        ]
                                    }.into_node(),
                                ],
                                ..Default::default()
                            }.into_node()
                        ]
                    }.into_node()
                )
                .padding_all(16.0)
                .bg(Color::WHITE)
                .border_radius(8.0)
                .shadow(BoxShadow { 
                    color: Color { r: 0, g: 0, b: 0, a: 20 }, 
                    blur_radius: 4.0, 
                    offset: (0.0, 2.0) 
                })
                .into_node();
    
                grid_items.push(card);
            }
        }

        // Grid layout simulation with Rows
        let rows: Vec<Node> = grid_items.chunks(3).map(|chunk| {
            Row {
                gap: Some(16.0),
                children: chunk.to_vec(),
                ..Default::default()
            }.into_node()
        }).collect();

        let content = Scroll {
            child: Some(Box::new(
                VStack {
                    spacing: Some(16.0),
                    children: rows,
                }.into_node()
            )),
            height: Some(600.0), 
            show_scrollbar: true,
            ..Default::default()
        };

        Container::new(
            VStack {
                spacing: Some(24.0),
                children: vec![
                    title.into_node(),
                    content.into_node(),
                ]
            }.into_node()
        )
        .padding_all(24.0)
        .bg(Color { r: 245, g: 245, b: 245, a: 255 })
        .flex_grow(1.0)
        .into_node()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(IconsApp);
    app.run()
}