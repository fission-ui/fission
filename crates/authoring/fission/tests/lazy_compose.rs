use fission::prelude::*;
use fission_core::authoring::BuildCtx;
use fission_core::{build, internal, Runtime, View};
use std::ops::ControlFlow;

fission_component! {
    LazyCounterRow<()> {
        props { title: &'static str }
        state { count: u32 = 0 }
    }
    view(row, _state, _cx) =>
        Button(
            child: Text(format!("{}: {}", row.title, row.count)),
            on_press: reduce { row.count += 1; },
        )
}

fn build_rows(runtime: &Runtime, titles: Vec<&'static str>) -> (Widget, BuildCtx<()>) {
    let env = Env::default();
    let state = runtime
        .get_global_state::<()>()
        .expect("the shell registers its application state before building");
    let view = View::new(state, &runtime.runtime_state, &env, None);
    let mut context = BuildCtx::new();
    let keys = titles.clone();
    let widget = build::enter(&mut context, &view, || {
        LazyColumn::with_keyed_item_builder(
            titles.len(),
            48.0,
            move |index| WidgetId::explicit(keys[index]),
            move |index| {
                LazyCounterRow {
                    title: titles[index],
                }
                .into()
            },
        )
        .id(WidgetId::explicit("compose-lazy-list"))
        .into()
    });
    // This mirrors the shell boundary: the row has already built and registered
    // its ordinary reducer before lowering starts outside the authoring scope.
    let _ = internal::lower_widget_to_ir(&widget);
    (widget, context)
}

#[test]
fn lazy_compose_rows_register_reducers_and_keep_keyed_state_after_reorder() {
    let mut runtime = Runtime::default().with_global_state(());
    let (widget, context) = build_rows(&runtime, vec!["A", "B"]);
    let mut actions = Vec::new();
    let _ = widget.visit(&mut |node| {
        if let Some(action) =
            internal::widget_as_button(node).and_then(|button| button.on_press.clone())
        {
            actions.push(action);
        }
        ControlFlow::Continue(())
    });
    assert_eq!(actions.len(), 2);
    runtime.absorb_registry(context.registry);
    runtime
        .dispatch(actions.remove(1), WidgetId::app_root())
        .expect("row reducer");
    let (reordered, _) = build_rows(&runtime, vec!["B", "A"]);
    let ir = internal::lower_widget_to_ir(&reordered);
    let text = ir
        .nodes
        .values()
        .flat_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                runs.iter().map(|run| run.text.clone()).collect()
            }
            Op::Paint(PaintOp::DrawText { text, .. }) => vec![text.clone()],
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert!(text.contains(&"B: 1".to_owned()), "{text:?}");
    assert!(text.contains(&"A: 0".to_owned()), "{text:?}");
}
