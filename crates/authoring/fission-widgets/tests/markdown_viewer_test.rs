use fission_core::internal::BuildCtx;
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget};
use fission_widgets::MarkdownViewer;

#[derive(Default, Debug, Clone)]
struct State;
impl GlobalState for State {}

fn build_markdown(markdown: &str) -> Widget {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();

    build::enter(&mut ctx, &view, || MarkdownViewer::new(markdown).into())
}

fn scroll_content(node: Widget) -> Widget {
    let scroll = fission_core::internal::widget_as_scroll(&node)
        .unwrap_or_else(|| panic!("expected MarkdownViewer to render a Scroll, got {node:?}"));
    scroll
        .child
        .as_ref()
        .expect("MarkdownViewer scroll content")
        .clone()
}

#[test]
fn renders_common_markdown_blocks_to_fission_nodes() {
    let node = build_markdown(
        "# Title\n\nA [link](https://example.com) with `code`.\n\n```rust\nlet x = 1;\n```\n\n- one\n- two\n\n> quoted\n\n---\n",
    );
    let content = scroll_content(node);

    let column = fission_core::internal::widget_as_column(&content)
        .expect("expected MarkdownViewer content to be a Column");
    assert_eq!(column.children.len(), 6);
    assert_eq!(
        fission_core::internal::widget_kind_name(&column.children[0]),
        "RichText"
    );
    assert_eq!(
        fission_core::internal::widget_kind_name(&column.children[2]),
        "Column"
    );
    assert_eq!(
        fission_core::internal::widget_kind_name(&column.children[3]),
        "Column"
    );
    assert_eq!(
        fission_core::internal::widget_kind_name(&column.children[4]),
        "Container"
    );
    assert_eq!(
        fission_core::internal::widget_kind_name(&column.children[5]),
        "Container"
    );

    let paragraph = fission_core::internal::widget_as_rich_text(&column.children[1])
        .expect("expected paragraph to render as RichText");
    assert!(paragraph
        .runs
        .iter()
        .any(|run| run.text == "link" && run.style.underline));
    assert!(paragraph
        .runs
        .iter()
        .any(|run| run.text == "code" && run.style.font_family.is_some()));

    let code = fission_core::internal::widget_as_column(&column.children[2])
        .expect("expected code block to carry semantics");
    let semantics = code.semantics.as_ref().expect("code block semantics");
    assert_eq!(
        semantics.identifier.as_deref(),
        Some("markdown-code-block:rust")
    );
    assert_eq!(semantics.value.as_deref(), Some("let x = 1;\n"));
}

#[test]
fn renders_gfm_table_as_rows_and_cells() {
    let node = build_markdown("| Name | Value |\n| --- | --- |\n| A | 1 |\n");
    let content = scroll_content(node);

    let column = fission_core::internal::widget_as_column(&content)
        .expect("expected MarkdownViewer content to be a Column");
    assert_eq!(column.children.len(), 1);

    let table = fission_core::internal::widget_as_container(&column.children[0])
        .expect("expected table to render as a Container");
    let Some(table_child) = &table.child else {
        panic!("expected table container to have child content");
    };
    let rows = fission_core::internal::widget_as_column(table_child)
        .expect("expected table content to be a Column of rows");
    assert_eq!(rows.children.len(), 2);
    assert!(rows
        .children
        .iter()
        .all(|row| fission_core::internal::widget_kind_name(row) == "Row"));
}
