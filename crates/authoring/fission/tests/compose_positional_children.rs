//! Constructor child arrays must convert each component inside its own key scope.

use fission::prelude::*;
use fission_core::authoring::BuildCtx;
use fission_core::{build, internal, Runtime, View};
use std::ops::ControlFlow;

fission_component! {
    CounterMetric<()> {
        props { label: &'static str }
        state { count: u32 = 0 }
    }
    view(metric, _state, _cx) =>
        Button(
            child: Text(format!("{}: {}", metric.label, metric.count)),
            on_press: reduce { metric.count += 1; },
        )
}

fission_component! {
    MetricGrid<()> {
        props { children: Vec<Widget> }
    }
    view(grid, _state, _cx) => Column(children: grid.children)
}

fission_component! {
    FixedMetrics<()>
    view(_metrics, _state, _cx) =>
        MetricGrid([
            CounterMetric("Site", key: "site"),
            CounterMetric("Asset", key: "asset"),
        ])
}

fission_component! {
    DynamicMetrics<()> {
        props { items: Vec<(&'static str, &'static str)> }
    }
    view(metrics, _state, _cx) =>
        MetricGrid([
            for (id, label) in metrics.items.iter() {
                CounterMetric(*label, key: *id)
            },
        ])
}

fn build_metrics(
    runtime: &Runtime,
    dynamic: bool,
    items: Vec<(&'static str, &'static str)>,
) -> (Widget, BuildCtx<()>) {
    let env = Env::default();
    let state = runtime.get_app_state::<()>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);
    let mut context = BuildCtx::new();
    let widget = build::enter(&mut context, &view, || {
        if dynamic {
            DynamicMetrics::new(items).into()
        } else {
            FixedMetrics.into()
        }
    });
    (widget, context)
}

#[test]
fn positional_child_arrays_keep_sibling_components_and_their_local_state_distinct() {
    for dynamic in [false, true] {
        let mut runtime = Runtime::default().with_global_state(());
        let (widget, context) = build_metrics(
            &runtime,
            dynamic,
            vec![("site", "Site"), ("asset", "Asset")],
        );
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
        assert_ne!(actions[0].id, actions[1].id);
        runtime.absorb_registry(context.registry);
        runtime
            .dispatch(actions.remove(0), WidgetId::app_root())
            .unwrap();

        let (rebuilt, _) = build_metrics(
            &runtime,
            dynamic,
            vec![("asset", "Equipment"), ("site", "Location")],
        );
        let ir = internal::lower_widget_to_ir(&rebuilt);
        let text = ir
            .nodes
            .values()
            .filter_map(|node| node.op.text())
            .collect::<Vec<_>>();
        let site = if dynamic { "Location: 1" } else { "Site: 1" };
        let asset = if dynamic { "Equipment: 0" } else { "Asset: 0" };
        assert!(text.iter().any(|value| value == site), "{text:?}");
        assert!(text.iter().any(|value| value == asset), "{text:?}");
    }
}
