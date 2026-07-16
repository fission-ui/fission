use fission_core::env::{Env, RuntimeState, SelectableTextState};
use fission_core::internal::{lower_widget, InternalLoweringCx};
use fission_core::ui::{
    ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion, Text, TextContent,
};
use fission_ir::{Op, PaintOp, Role, WidgetId};
use fission_layout::LayoutPoint;

#[test]
fn selectable_text_lowers_semantics_and_runtime_selection() {
    let env = Env::default();
    let text_id = WidgetId::explicit("selectable.text");
    let mut runtime = RuntimeState::default();
    runtime.selectable_text.states.insert(
        text_id,
        SelectableTextState {
            anchor: 0,
            caret: 5,
            selecting: false,
        },
    );

    let text = Text {
        id: Some(text_id),
        content: TextContent::Literal("hello world".into()),
        selectable: true,
        ..Default::default()
    };

    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let root_id = lower_widget(&text.into(), &mut cx);
    cx.ir.root = Some(root_id);

    let semantics = cx
        .ir
        .nodes
        .get(&text_id)
        .and_then(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("selectable text should lower to a semantics owner");
    assert_eq!(semantics.role, Role::Text);
    assert!(semantics.selectable_text);
    assert!(semantics.context_menu);
    assert!(semantics.focusable);
    assert!(semantics.read_only);
    assert_eq!(semantics.value.as_deref(), Some("hello world"));
    assert_eq!(semantics.text_selection, Some((0, 5)));

    let has_selection_paint = cx.ir.nodes.values().any(|node| match &node.op {
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs
            .iter()
            .any(|run| !run.text.is_empty() && run.style.background_color.is_some()),
        _ => false,
    });
    assert!(
        has_selection_paint,
        "selected text should paint as rich text"
    );
}

#[test]
fn context_menu_region_accepts_widget_children_and_fallback_i18n_text() {
    let env = Env::default();
    let menu_id = WidgetId::explicit("menu.region");
    let mut runtime = RuntimeState::default();
    runtime
        .context_menu
        .open(menu_id, LayoutPoint::new(12.0, 18.0));

    let menu = ContextMenu::with_items([ContextMenuEntry::Item(ContextMenuItem::new(
        "copy",
        Text::new(TextContent::KeyWithFallback {
            key: "menu.copy".into(),
            fallback: "Copy".into(),
        }),
    ))]);
    let region = ContextMenuRegion::new(Text::new("target"), menu).id(menu_id);

    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let root_id = lower_widget(&region.into(), &mut cx);
    cx.ir.root = Some(root_id);

    let semantics = cx
        .ir
        .nodes
        .get(&menu_id)
        .and_then(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("context menu region should lower to semantics");
    assert!(semantics.context_menu);

    let rendered_fallback = cx.ir.nodes.values().any(|node| match &node.op {
        Op::Paint(PaintOp::DrawText { text, .. }) => text == "Copy",
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs.iter().any(|run| run.text == "Copy"),
        _ => false,
    });
    assert!(
        rendered_fallback,
        "menu item child should render fallback text"
    );
}
