use fission_core::ui::{Button, ButtonVariant, Checkbox, Container, Node, Text, Scroll, Row, Column};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId, NodeId};
use fission_core::op::{Color, BoxShadow};
use crate::stack::{VStack, HStack};
use crate::{Icon, MenuButton, MenuItem};
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableColumn {
    pub id: String,
    pub title: String,
    pub width: f32,
    pub sortable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableRow {
    pub id: String,
    pub cells: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataTable {
    pub id: WidgetNodeId,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRow>,
    pub selected_ids: Vec<String>,
    pub on_selection_change: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for DataTable {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        // Header
        let mut header_cells = Vec::new();
        // Checkbox column
        header_cells.push(
            Container::new(
                Checkbox { 
                    checked: false, 
                    label: None,
                    on_toggle: None,
                    ..Default::default()
                }.into_node()
            )
            .width(40.0)
            .padding_all(8.0)
            .into_node()
        );

        for col in &self.columns {
            header_cells.push(
                Container::new(
                    HStack {
                        spacing: Some(4.0),
                        children: vec![
                            Text::new(col.title.clone())
                                // .weight(fission_core::ui::FontWeight::Bold) // Stubbed
                                .color(tokens.colors.text_secondary)
                                .size(12.0)
                                .into_node(),
                            if col.sortable {
                                Icon::svg(material::navigation::arrow_drop_down::regular())
                                    .size(16.0)
                                    .color(tokens.colors.text_secondary)
                                    .into_node()
                            } else {
                                fission_core::ui::widgets::Spacer { width: Some(16.0), ..Default::default() }.into_node()
                            }
                        ]
                    }.into_node()
                )
                .width(col.width)
                .padding_all(8.0)
                .into_node()
            );
        }

        let header = Container::new(
            HStack {
                spacing: Some(0.0),
                children: header_cells,
            }.into_node()
        )
        .bg(tokens.colors.surface)
        .flex_shrink(0.0) // Header shouldn't shrink
        .into_node();

        // Rows
        let mut row_nodes = Vec::new();
        for row in &self.rows {
            let is_selected = self.selected_ids.contains(&row.id);
            let mut row_cells = Vec::new();
            
            // Checkbox
            row_cells.push(
                Container::new(
                    Checkbox { 
                        checked: is_selected,
                        label: None,
                        on_toggle: None, 
                        ..Default::default()
                    }.into_node()
                )
                .width(40.0)
                .padding_all(8.0)
                .into_node()
            );

            for (i, cell_text) in row.cells.iter().enumerate() {
                let width = self.columns.get(i).map(|c| c.width).unwrap_or(100.0);
                row_cells.push(
                    Container::new(
                        Text::new(cell_text.clone())
                            .size(14.0)
                            .color(tokens.colors.text_primary)
                            .into_node()
                    )
                    .width(width)
                    .padding_all(8.0)
                    .into_node()
                );
            }

            let row_content = HStack {
                spacing: Some(0.0),
                children: row_cells,
            }.into_node();

            row_nodes.push(
                Container::new(row_content)
                    .bg(if is_selected { tokens.colors.primary.with_alpha(20) } else { Color::WHITE })
                    .into_node()
            );
            
            // Divider
            row_nodes.push(
                Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                    .height(1.0)
                    .bg(tokens.colors.border)
                    .into_node()
            );
        }

        let content = Scroll {
            child: Some(Box::new(
                VStack {
                    spacing: Some(0.0),
                    children: row_nodes,
                }.into_node()
            )),
            show_scrollbar: true,
            ..Default::default()
        }.into_node();

        Container::new(
            VStack {
                spacing: Some(0.0),
                children: vec![header, content],
            }.into_node()
        )
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.small)
        .into_node()
    }
}