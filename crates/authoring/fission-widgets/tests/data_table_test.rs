use fission_core::authoring::{lower_widget_to_ir_in, BuildCtx};
use fission_core::{build, Env, GlobalState, View, Widget};
use fission_ir::{CoreIR, Op, Role, Semantics};
use fission_widgets::{DataTable, TableColumn, TableRow};

#[derive(Default, Debug)]
struct TestState;
impl GlobalState for TestState {}

fn lower(env: &Env, build_widget: impl FnOnce() -> Widget) -> CoreIR {
    let state = TestState;
    let runtime = fission_core::RuntimeState::default();
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    lower_widget_to_ir_in(env, &widget)
}

fn roles(ir: &CoreIR, role: Role) -> Vec<&Semantics> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == role => Some(semantics),
            _ => None,
        })
        .collect()
}

fn table() -> DataTable {
    DataTable {
        columns: vec![
            TableColumn {
                id: "name".into(),
                title: "Name".into(),
                width: 120.0,
                sortable: true,
                ..Default::default()
            },
            TableColumn {
                id: "email".into(),
                title: "Email".into(),
                width: 200.0,
                ..Default::default()
            },
        ],
        rows: vec![
            TableRow {
                id: "1".into(),
                cells: vec!["Ada".into(), "ada@example.com".into()],
            },
            TableRow {
                id: "2".into(),
                cells: vec!["Grace".into(), "grace@example.com".into()],
            },
        ],
        label: Some("People".into()),
        ..Default::default()
    }
}

#[test]
fn data_table_exposes_table_row_and_cell_semantics() {
    let ir = lower(&Env::default(), || table().into());

    let tables = roles(&ir, Role::Table);
    assert_eq!(tables.len(), 1, "a data table should expose one table node");
    assert_eq!(tables[0].label.as_deref(), Some("People"));

    let headers = roles(&ir, Role::ColumnHeader);
    assert_eq!(headers.len(), 2, "each column needs a header node");
    assert!(headers.iter().any(|h| h.label.as_deref() == Some("Name")));

    // One header row plus one row per record.
    assert_eq!(roles(&ir, Role::TableRow).len(), 3);
    assert_eq!(
        roles(&ir, Role::TableCell).len(),
        4,
        "two records of two cells each"
    );
}

#[test]
fn selected_rows_report_selection() {
    let mut component = table();
    component.selected_ids = vec!["2".into()];
    let ir = lower(&Env::default(), || component.into());

    let selected = roles(&ir, Role::TableRow)
        .into_iter()
        .filter(|row| row.selected == Some(true))
        .count();
    assert_eq!(
        selected, 1,
        "only the selected record should report selection"
    );
}

#[test]
fn rows_do_not_paint_a_fixed_white_background() {
    // A row painted pure white is unreadable in a dark theme, so the surface
    // colour has to come from the active token set.
    use fission_ir::op::{Color, Fill, PaintOp};

    let env = Env {
        theme: fission_theme::Theme::dark(),
        ..Env::default()
    };
    let ir = lower(&env, || table().into());
    let white = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    assert!(
        !ir.nodes.values().any(|node| matches!(
            &node.op,
            Op::Paint(PaintOp::DrawRect { fill: Some(Fill::Solid(colour)), .. }) if *colour == white
        )),
        "a dark theme must not paint row surfaces white"
    );
}
