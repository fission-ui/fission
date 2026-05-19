use fission_core::{AppState, BuildCtx, Env, Node, RuntimeState, View, Widget};
use fission_widgets::MarkdownViewer;

#[derive(Default, Debug, Clone)]
struct State;
impl AppState for State {}

fn build_markdown(markdown: &str) -> Node {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();

    MarkdownViewer::new(markdown).build(&mut ctx, &view)
}

fn scroll_content(node: Node) -> Node {
    match node {
        Node::Scroll(scroll) => *scroll.child.expect("MarkdownViewer scroll content"),
        other => panic!("expected MarkdownViewer to render a Scroll, got {other:?}"),
    }
}

#[test]
fn renders_common_markdown_blocks_to_fission_nodes() {
    let node = build_markdown(
        "# Title\n\nA [link](https://example.com) with `code`.\n\n```rust\nlet x = 1;\n```\n\n- one\n- two\n\n> quoted\n\n---\n",
    );
    let content = scroll_content(node);

    let Node::Column(column) = content else {
        panic!("expected MarkdownViewer content to be a Column");
    };
    assert_eq!(column.children.len(), 6);
    assert!(matches!(column.children[0], Node::RichText(_)));
    assert!(matches!(column.children[2], Node::Container(_)));
    assert!(matches!(column.children[3], Node::Column(_)));
    assert!(matches!(column.children[4], Node::Container(_)));
    assert!(matches!(column.children[5], Node::Container(_)));

    let Node::RichText(paragraph) = &column.children[1] else {
        panic!("expected paragraph to render as RichText");
    };
    assert!(paragraph
        .runs
        .iter()
        .any(|run| run.text == "link" && run.style.underline));
    assert!(paragraph
        .runs
        .iter()
        .any(|run| run.text == "code" && run.style.font_family.is_some()));
}

#[test]
fn renders_gfm_table_as_rows_and_cells() {
    let node = build_markdown("| Name | Value |\n| --- | --- |\n| A | 1 |\n");
    let content = scroll_content(node);

    let Node::Column(column) = content else {
        panic!("expected MarkdownViewer content to be a Column");
    };
    assert_eq!(column.children.len(), 1);

    let Node::Container(table) = &column.children[0] else {
        panic!("expected table to render as a Container");
    };
    let Some(table_child) = &table.child else {
        panic!("expected table container to have child content");
    };
    let Node::Column(rows) = table_child.as_ref() else {
        panic!("expected table content to be a Column of rows");
    };
    assert_eq!(rows.children.len(), 2);
    assert!(rows.children.iter().all(|row| matches!(row, Node::Row(_))));
}
