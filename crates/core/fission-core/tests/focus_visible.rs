//! Focus is drawn after keyboard navigation but not after a click.

use fission_core::{Runtime, TextEditSource, WidgetId};
use fission_ir::CoreIR;

#[test]
fn keyboard_focus_is_drawn_and_pointer_focus_is_not() {
    let mut runtime = Runtime::default();
    let ir = CoreIR::default();
    let first = WidgetId::explicit("focus.first");
    let second = WidgetId::explicit("focus.second");

    runtime
        .set_focused_widget(&ir, Some(first), TextEditSource::Pointer)
        .unwrap();
    assert!(runtime.runtime_state.interaction.is_focused(first));
    assert!(!runtime.runtime_state.interaction.is_focus_visible(first));

    runtime
        .set_focused_widget(&ir, Some(second), TextEditSource::Keyboard)
        .unwrap();
    assert!(runtime.runtime_state.interaction.is_focus_visible(second));

    runtime
        .set_focused_widget(&ir, Some(first), TextEditSource::Programmatic)
        .unwrap();
    assert!(
        runtime.runtime_state.interaction.is_focus_visible(first),
        "programmatic focus keeps the modality the user was last using"
    );
}

#[test]
fn a_press_hides_focus_and_keyboard_navigation_shows_it_without_moving_focus() {
    use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
    use fission_layout::{LayoutPoint, LayoutSize, LayoutSnapshot};

    let mut runtime = Runtime::default();
    let ir = CoreIR::default();
    let layout = LayoutSnapshot::new(LayoutSize::new(400.0, 300.0));
    let focused = WidgetId::explicit("focus.focused");
    runtime
        .set_focused_widget(&ir, Some(focused), TextEditSource::Keyboard)
        .unwrap();
    assert!(runtime.runtime_state.interaction.focus_visible);

    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Down {
                pointer_id: Default::default(),
                kind: Default::default(),
                point: LayoutPoint::new(10.0, 10.0),
                button: PointerButton::Primary,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .unwrap();
    assert!(
        !runtime.runtime_state.interaction.focus_visible,
        "a press hides focus rings even when focus does not move"
    );

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Tab,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .unwrap();
    assert!(
        runtime.runtime_state.interaction.focus_visible,
        "keyboard navigation shows focus rings again"
    );
}

#[test]
fn custom_ancestor_blurs_only_when_focus_leaves_its_subtree() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use fission_core::authoring::{IrBuilder, LowerWidget, LoweringContext};
    use fission_core::internal::{CustomRender, CustomRenderObject};
    use fission_core::ui::Widget;
    use fission_ir::{Op, Role, Semantics};

    #[derive(Debug)]
    struct BlurRecorder(Arc<AtomicUsize>);

    impl CustomRenderObject for BlurRecorder {
        fn blur_actions(
            &self,
            _node_id: WidgetId,
        ) -> Vec<(WidgetId, fission_core::ActionEnvelope)> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }
    }

    #[derive(Debug)]
    struct RegionLowerer;

    impl LowerWidget for RegionLowerer {
        fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
            let first = IrBuilder::new(
                WidgetId::explicit("focus.region.first"),
                Op::Semantics(Semantics {
                    role: Role::Button,
                    focusable: true,
                    ..Default::default()
                }),
            )
            .build(cx);
            let second = IrBuilder::new(
                WidgetId::explicit("focus.region.second"),
                Op::Semantics(Semantics {
                    role: Role::Button,
                    focusable: true,
                    ..Default::default()
                }),
            )
            .build(cx);
            let mut region = IrBuilder::new(
                WidgetId::explicit("focus.region"),
                Op::Semantics(Semantics {
                    role: Role::Group,
                    ..Default::default()
                }),
            );
            region.add_child(first);
            region.add_child(second);
            region.build(cx)
        }

        fn widget_id(&self) -> Option<WidgetId> {
            Some(WidgetId::explicit("focus.region.wrapper"))
        }
    }

    let blur_count = Arc::new(AtomicUsize::new(0));
    let widget: Widget = CustomRender::new("focus-test-region", Arc::new(RegionLowerer))
        .with_render_object(Arc::new(BlurRecorder(blur_count.clone())))
        .into();
    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    let first = WidgetId::explicit("focus.region.first");
    let second = WidgetId::explicit("focus.region.second");
    let mut runtime = Runtime::default();

    runtime
        .set_focused_widget(&ir, Some(first), TextEditSource::Programmatic)
        .unwrap();
    runtime
        .set_focused_widget(&ir, Some(second), TextEditSource::Programmatic)
        .unwrap();
    assert_eq!(blur_count.load(Ordering::SeqCst), 0);

    runtime
        .set_focused_widget(&ir, None, TextEditSource::Programmatic)
        .unwrap();
    assert_eq!(blur_count.load(Ordering::SeqCst), 1);
}
