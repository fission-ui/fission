use crate::model::EditorState;
use fission::core::op::Color;
use fission::core::ui::{Container, Node, Text};
use fission::core::{BuildCtx, View, Widget};
use fission::widgets::{HStack, Spacer};

#[derive(Clone)]
pub struct Breadcrumb;

impl Widget<EditorState> for Breadcrumb {
    fn build(&self, _ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> impl fission::IntoWidget<EditorState> {
        fission::core::view::internal_node_widget({

        
        let Some((tab, _buf)) = view.state.active_buffer() else {
            return fission::core::view::internal_node_widget(Spacer { height: Some(0.0), ..Default::default() }.into_node());
        };

        let dim = Color { r: 140, g: 140, b: 140, a: 255 };
        let sep_color = Color { r: 100, g: 100, b: 100, a: 255 };
        let text_color = Color { r: 190, g: 190, b: 190, a: 255 };

        // Build breadcrumb from file path relative to root
        let root_str = view.state.root_path.to_string_lossy().to_string();
        let relative = if tab.path.starts_with(&root_str) {
            tab.path[root_str.len()..].trim_start_matches('/').to_string()
        } else {
            tab.path.clone()
        };

        let segments: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();

        let mut children = Vec::new();
        for (i, seg) in segments.iter().enumerate() {
            if i > 0 {
                children.push(
                    Text::new(">").size(10.0).color(sep_color).into_node(),
                );
            }
            let color = if i == segments.len() - 1 { text_color } else { dim };
            children.push(
                Text::new(*seg).size(11.0).color(color).into_node(),
            );
        }

        Container::<fission::Node>::lowered(
            HStack {
                spacing: Some(4.0),
                children,
            }.into_node(),
        )
        .bg(Color { r: 30, g: 30, b: 30, a: 255 })
        .height(22.0)
        .padding_all(4.0)
        .flex_shrink(0.0)
        .into_node()
    
        
    
        })
    }
}
