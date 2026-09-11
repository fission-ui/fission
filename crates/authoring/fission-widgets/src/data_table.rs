use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::ui::{
    Button, ButtonVariant, Checkbox, Container, Scroll, SemanticsRegion, Text, Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::Role;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Declarative description of one data-table column.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableColumn {
    /// Stable application identifier for sorting and column-specific behavior.
    pub id: String,
    /// Header label.
    pub title: String,
    /// Logical width allocated to each cell in the column.
    pub width: f32,
    /// Whether the header should expose sorting affordance.
    pub sortable: bool,
    /// Action dispatched when this column's header is activated.
    ///
    /// A sortable column with no action renders its affordance disabled rather
    /// than showing a control that does nothing when pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_sort: Option<ActionEnvelope>,
    /// Current sort direction, when this column is the active sort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sorted_ascending: Option<bool>,
}

/// One table row containing cells in the same order as the column declarations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableRow {
    /// Stable application identifier used for controlled selection.
    pub id: String,
    /// Preformatted cell text corresponding positionally to table columns.
    pub cells: Vec<String>,
}

impl Default for TableColumn {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            width: 120.0,
            sortable: false,
            on_sort: None,
            sorted_ascending: None,
        }
    }
}

/// Scrollable controlled data table with row selection.
#[derive(Clone)]
pub struct DataTable {
    /// Stable widget identity used by retained interaction state.
    pub id: WidgetId,
    /// Column definitions in display order.
    pub columns: Vec<TableColumn>,
    /// Row values in display order.
    pub rows: Vec<TableRow>,
    /// Application-owned set of selected row IDs.
    pub selected_ids: Vec<String>,
    /// Factory that produces an action when a row selection is toggled.
    pub on_selection_change: Option<Arc<dyn Fn(String) -> ActionEnvelope + Send + Sync>>,
    /// Action dispatched when the header's select-all control is toggled.
    ///
    /// Without this the header control is rendered disabled, rather than
    /// offering a checkbox that silently does nothing.
    pub on_select_all: Option<ActionEnvelope>,
    /// Accessible name for the table.
    pub label: Option<String>,
}

impl Default for DataTable {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("data-table"),
            columns: Vec::new(),
            rows: Vec::new(),
            selected_ids: Vec::new(),
            on_selection_change: None,
            on_select_all: None,
            label: None,
        }
    }
}

impl std::fmt::Debug for DataTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataTable")
            .field("id", &self.id)
            .field("columns", &self.columns)
            .field("rows_len", &self.rows.len())
            .field("selected_ids_len", &self.selected_ids.len())
            .finish()
    }
}

impl From<DataTable> for Widget {
    fn from(component: DataTable) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;

        // Header
        let mut header_cells = Vec::new();
        // Checkbox column
        let all_selected = !this.rows.is_empty()
            && this
                .rows
                .iter()
                .all(|row| this.selected_ids.contains(&row.id));
        header_cells.push(
            Container::new(
                SemanticsRegion::new(Checkbox {
                    checked: all_selected,
                    label: None,
                    on_toggle: this.on_select_all.clone(),
                    disabled: this.on_select_all.is_none(),
                    ..Default::default()
                })
                .label("Select all rows"),
            )
            .width(40.0)
            .padding_all(8.0)
            .into(),
        );

        for col in &this.columns {
            let label = Text::new(col.title.clone())
                .color(tokens.colors.text_secondary)
                .size(tokens.typography.font_size_base)
                .weight(tokens.typography.font_weight_medium);
            let affordance: Widget = if col.sortable {
                // The glyph reports the direction currently applied, so the
                // header states the table's order rather than only offering to
                // change it.
                let glyph = match col.sorted_ascending {
                    Some(true) => material::navigation::arrow_drop_up::regular(),
                    _ => material::navigation::arrow_drop_down::regular(),
                };
                Icon::svg(glyph)
                    .size(tokens.spacing.m)
                    .color(if col.sorted_ascending.is_some() {
                        tokens.colors.text_primary
                    } else {
                        tokens.colors.text_secondary
                    })
                    .into()
            } else {
                fission_core::ui::widgets::Spacer {
                    width: Some(tokens.spacing.m),
                    ..Default::default()
                }
                .into()
            };
            let content: Widget = HStack {
                spacing: Some(tokens.spacing.xs),
                children: vec![label.into(), affordance],
            }
            .into();
            let cell: Widget = if col.sortable && col.on_sort.is_some() {
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(content),
                    on_press: col.on_sort.clone(),
                    ..Default::default()
                }
                .into()
            } else {
                content
            };
            header_cells.push(
                SemanticsRegion::new(
                    Container::new(cell)
                        .width(col.width)
                        .padding_all(tokens.spacing.s),
                )
                .role(Role::ColumnHeader)
                .label(col.title.clone())
                .into(),
            );
        }

        let header: Widget = SemanticsRegion::new(
            Container::new(HStack {
                spacing: Some(0.0),
                children: header_cells,
            })
            .bg(tokens.colors.surface)
            .flex_shrink(0.0), // Header shouldn't shrink
        )
        .role(Role::TableRow)
        .into();

        // Rows
        let mut row_nodes = Vec::new();
        for row in &this.rows {
            let is_selected = this.selected_ids.contains(&row.id);
            let mut row_cells = Vec::new();

            // Checkbox
            let toggle = this.on_selection_change.clone();
            row_cells.push(
                Container::new(
                    SemanticsRegion::new(Checkbox {
                        checked: is_selected,
                        label: None,
                        on_toggle: toggle.map(|f| f(row.id.clone())),
                        ..Default::default()
                    })
                    .label(format!("Select row {}", row.id)),
                )
                .width(40.0)
                .padding_all(tokens.spacing.s)
                .into(),
            );

            for (i, cell_text) in row.cells.iter().enumerate() {
                let width = this.columns.get(i).map(|c| c.width).unwrap_or(100.0);
                row_cells.push(
                    SemanticsRegion::new(
                        Container::new(
                            Text::new(cell_text.clone())
                                .size(tokens.typography.body_medium_size)
                                .color(tokens.colors.text_primary),
                        )
                        .width(width)
                        .padding_all(tokens.spacing.s),
                    )
                    .role(Role::TableCell)
                    .into(),
                );
            }

            let row_content: Widget = HStack {
                spacing: Some(0.0),
                children: row_cells,
            }
            .into();

            let row_toggle = this.on_selection_change.clone().map(|f| f(row.id.clone()));
            let row_body = Container::new(row_content)
                .bg(if is_selected {
                    tokens.colors.primary.with_alpha(20)
                } else {
                    // Previously Color::WHITE, which painted every row white in
                    // a dark theme and left the text unreadable.
                    tokens.colors.surface
                })
                .into();
            let row_node = if let Some(action) = row_toggle {
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(row_body),
                    on_press: Some(action),
                    ..Default::default()
                }
                .into()
            } else {
                row_body
            };
            row_nodes.push(
                SemanticsRegion::new(row_node)
                    .role(Role::TableRow)
                    .selected(is_selected)
                    .into(),
            );

            // Divider
            row_nodes.push(
                Container::new(fission_core::ui::widgets::Spacer::default())
                    .height(1.0)
                    .bg(tokens.colors.divider)
                    .into(),
            );
        }

        let content = Scroll {
            child: Some(
                VStack {
                    spacing: Some(0.0),
                    children: row_nodes,
                }
                .into(),
            ),
            show_scrollbar: true,
            ..Default::default()
        }
        .into();

        let mut table = SemanticsRegion::new(
            Container::new(VStack {
                spacing: Some(0.0),
                children: vec![header, content],
            })
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.small),
        )
        .role(Role::Table);
        if let Some(label) = this.label.clone() {
            table = table.label(label);
        }
        table.into()
    }
}
