use fission_core::{ActionRegistry, GlobalState, ReducerContext, Runtime, ViewportResized};
use fission_layout::LayoutSize;

#[derive(Debug, Default)]
struct ResizeState {
    sizes: Vec<LayoutSize>,
}

impl GlobalState for ResizeState {}

fn record_resize(
    state: &mut ResizeState,
    resize: ViewportResized,
    _context: &mut ReducerContext<ResizeState>,
) {
    state.sizes.push(resize.size);
}

#[test]
fn viewport_resize_handler_runs_only_for_committed_size_changes() {
    let mut runtime = Runtime::default().with_global_state(ResizeState::default());
    let mut registry = ActionRegistry::<ResizeState>::new();
    let handler: fission_core::Handler<ResizeState, ViewportResized> = record_resize;
    registry.register::<ViewportResized, _>(handler);
    runtime.absorb_persistent_registry(registry);

    let initial = LayoutSize::new(960.0, 540.0);
    let phone = LayoutSize::new(390.0, 844.0);
    assert!(!runtime
        .dispatch_viewport_resize_if_changed(None, initial)
        .unwrap());
    assert!(!runtime
        .dispatch_viewport_resize_if_changed(Some(initial), initial)
        .unwrap());
    assert!(runtime
        .dispatch_viewport_resize_if_changed(Some(initial), phone)
        .unwrap());

    assert_eq!(
        runtime.get_global_state::<ResizeState>().unwrap().sizes,
        [phone]
    );
}
