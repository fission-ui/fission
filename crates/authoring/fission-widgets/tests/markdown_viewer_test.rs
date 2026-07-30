use fission_core::internal::BuildCtx;
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget};
use fission_ir::op::{ImageSource, PaintOp};
use fission_ir::Op;
use fission_widgets::{MarkdownContent, MarkdownViewer};

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

fn build_markdown_content(markdown: &str) -> Widget {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();

    build::enter(&mut ctx, &view, || MarkdownContent::new(markdown).into())
}

#[test]
fn markdown_content_participates_in_parent_scrolling_without_nested_scroll() {
    let content = build_markdown_content("# Title\n\nBody\n");
    assert!(
        fission_core::internal::widget_as_scroll(&content).is_none(),
        "MarkdownContent must not create an independent scroll viewport"
    );
    let column = fission_core::internal::widget_as_column(&content)
        .expect("MarkdownContent should render its document column directly");
    assert_eq!(column.children.len(), 2);
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
fn disables_unsafe_link_schemes_without_hiding_link_text() {
    let node = build_markdown_content(
        "[safe](https://example.com) [relative](./details) [unsafe](javascript:alert(1))\n",
    );
    let column = fission_core::internal::widget_as_column(&node)
        .expect("expected MarkdownContent to render a Column");
    let paragraph = fission_core::internal::widget_as_rich_text(&column.children[0])
        .expect("expected paragraph to render as RichText");

    let safe = paragraph
        .runs
        .iter()
        .find(|run| run.text == "safe")
        .expect("safe link text");
    assert!(safe.style.underline);

    let relative = paragraph
        .runs
        .iter()
        .find(|run| run.text == "relative")
        .expect("relative link text");
    assert!(relative.style.underline);

    let unsafe_link = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("unsafe"))
        .expect("unsafe link text remains visible");
    assert!(!unsafe_link.style.underline);
    assert!(unsafe_link.semantics_identifier.is_none());
}

#[test]
fn disables_unsafe_link_wrappers_around_images() {
    let node = build_markdown_content(
        "[![Screenshot](https://cdn.example.com/image.png)](javascript:alert(1))\n",
    );
    let ir = fission_core::internal::lower_widget_to_ir(&node);

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawImage { request, .. })
            if matches!(
                &request.source,
                ImageSource::Network { url, .. }
                    if url == "https://cdn.example.com/image.png"
            )
    )));
    assert!(!ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Semantics(semantics)
            if semantics
                .identifier
                .as_deref()
                .is_some_and(|identifier| identifier.starts_with("markdown-link:javascript:"))
    )));
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

#[test]
fn renders_linked_markdown_images_as_image_links() {
    let node = build_markdown(
        "[![Bluetooth screenshot](https://cdn.example.com/thumb.png)](https://cdn.example.com/full.png)\n",
    );
    let content = scroll_content(node);
    let ir = fission_core::internal::lower_widget_to_ir(&content);

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawImage { request, .. })
            if matches!(
                &request.source,
                ImageSource::Network { url, .. } if url == "https://cdn.example.com/thumb.png"
            )
            && request.semantic_label.as_deref() == Some("Bluetooth screenshot")
    )));
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Semantics(semantics)
            if semantics.identifier.as_deref()
                == Some("markdown-link:https://cdn.example.com/full.png")
    )));
}

#[test]
fn renders_image_only_paragraphs_as_wrapping_galleries() {
    let node = build_markdown(
        "[![One](https://cdn.example.com/one.png)](https://cdn.example.com/one-full.png) [![Two](https://cdn.example.com/two.png)](https://cdn.example.com/two-full.png)\n",
    );
    let content = scroll_content(node);

    let column = fission_core::internal::widget_as_column(&content)
        .expect("expected MarkdownViewer content to be a Column");
    assert_eq!(
        fission_core::internal::widget_kind_name(&column.children[0]),
        "Row"
    );

    let ir = fission_core::internal::lower_widget_to_ir(&content);
    let image_count = ir
        .nodes
        .values()
        .filter(|node| matches!(&node.op, Op::Paint(PaintOp::DrawImage { .. })))
        .count();
    assert_eq!(image_count, 2);
}
