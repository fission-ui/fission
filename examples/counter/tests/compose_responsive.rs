use fission::prelude::*;
use fission_test::TestHarness;

fission_component! {
    ResponsiveBranch<()> {
        props { label: &'static str }
        state { count: i32 = 0 }
    }
    view(branch, _state, _cx) =>
        Text(format!("{}: {}", branch.label, branch.count))
}

fission_component! {
    ResponsiveCounter<()>
    view(_counter, _state, _cx) =>
        Responsive::new_with_id(
            WidgetId::explicit("counter.responsive"),
            || ResponsiveBranch::new("compact"),
        )
        .case_min_width_with(720.0, || ResponsiveBranch::new("wide"))
}

#[test]
fn responsive_branches_scope_retained_compose_components() {
    let mut harness = TestHarness::new(()).with_root_widget(ResponsiveCounter::new());
    harness.pump().expect("pump responsive counter");

    assert!(harness.last_ir.is_some());
}
