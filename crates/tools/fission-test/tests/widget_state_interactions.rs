use anyhow::Result;
use fission_core::ui::{Node, Text};
use fission_core::{
    ActionEnvelope, ActionId, AnimationPropertyId, AppState, BuildCtx, View, Widget, WidgetNodeId,
};
use fission_ir::semantics::{Role, TextInputType};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{DatePicker, Drawer, DrawerSide, NumberInput};
use std::sync::Arc;

const NUMBER_CHANGED_ID: ActionId = ActionId::from_u128(0xF151_0001);
const DATE_NAVIGATED_ID: ActionId = ActionId::from_u128(0xF151_0002);
const DRAWER_DISMISSED_ID: ActionId = ActionId::from_u128(0xF151_0003);

#[derive(Debug, Clone)]
struct State {
    number: f32,
    date_year: i32,
    date_month: u32,
    drawer_open: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            number: 0.0,
            date_year: 2026,
            date_month: 5,
            drawer_open: true,
        }
    }
}

impl AppState for State {}

#[test]
fn number_input_text_entry_dispatches_parsed_float() -> Result<()> {
    struct Root;

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            ctx.registry.register_raw_action(
                NUMBER_CHANGED_ID,
                |state, envelope, _target, _effects, _input| {
                    state.number = serde_json::from_slice::<f32>(&envelope.payload)?;
                    Ok(())
                },
            );

            NumberInput {
                id: Some(WidgetNodeId::explicit("quantity")),
                value: view.state.number,
                display_text: Some(String::new()),
                on_change: Some(ActionEnvelope {
                    id: NUMBER_CHANGED_ID,
                    payload: Vec::new(),
                }),
                ..Default::default()
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    let inputs = driver.find_role(Role::TextInput);
    assert_eq!(inputs.len(), 1, "NumberInput should expose one text field");
    let input_node = inputs[0].node_id;
    let input_semantics = driver
        .harness
        .last_ir
        .as_ref()
        .and_then(|ir| ir.nodes.get(&input_node))
        .and_then(|node| match &node.op {
            fission_ir::Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("NumberInput semantics");
    assert_eq!(input_semantics.text_input_type, TextInputType::Number);

    let bounds = inputs[0].bounds;
    driver.tap_point(
        bounds.x() + bounds.width() / 2.0,
        bounds.y() + bounds.height() / 2.0,
    )?;
    driver.type_text("12.5")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.number, 12.5);

    Ok(())
}

#[test]
fn date_picker_navigation_is_controlled_by_parent_state() -> Result<()> {
    struct Root;

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            ctx.registry.register_raw_action(
                DATE_NAVIGATED_ID,
                |state, envelope, _target, _effects, _input| {
                    let (year, month) = serde_json::from_slice::<(i32, u32)>(&envelope.payload)?;
                    state.date_year = year;
                    state.date_month = month;
                    Ok(())
                },
            );

            DatePicker {
                id: WidgetNodeId::explicit("due_date"),
                value: None,
                is_open: true,
                width: Some(180.0),
                view_year: Some(view.state.date_year),
                view_month: Some(view.state.date_month),
                on_navigate: Some(Arc::new(|year, month| ActionEnvelope {
                    id: DATE_NAVIGATED_ID,
                    payload: serde_json::to_vec(&(year, month)).unwrap(),
                })),
                on_change: None,
                on_toggle: None,
                on_close: None,
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;
    driver.assert_text_visible("May 2026");

    driver.tap_text(">")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!((state.date_year, state.date_month), (2026, 6));
    driver.assert_text_visible("June 2026");

    Ok(())
}

#[test]
fn drawer_backdrop_dismisses_and_registers_enter_animation() -> Result<()> {
    let drawer_id = WidgetNodeId::explicit("settings_drawer");

    struct Root {
        drawer_id: WidgetNodeId,
    }

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            ctx.registry.register_raw_action(
                DRAWER_DISMISSED_ID,
                |state, _envelope, _target, _effects, _input| {
                    state.drawer_open = false;
                    Ok(())
                },
            );

            Drawer {
                id: self.drawer_id,
                side: DrawerSide::Left,
                is_open: view.state.drawer_open,
                on_dismiss: Some(ActionEnvelope {
                    id: DRAWER_DISMISSED_ID,
                    payload: Vec::new(),
                }),
                content: Box::new(Text::new("Drawer content").into_node()),
                width: Some(300.0),
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { drawer_id });
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;
    driver.assert_text_visible("Drawer content");

    let slide_anim_id = WidgetNodeId::from_u128(drawer_id.as_u128() ^ 0xD00D_u128);
    let active = driver
        .harness
        .runtime
        .runtime_state
        .animation
        .active
        .get(&(slide_anim_id, AnimationPropertyId::TranslateX))
        .expect("drawer slide animation");
    assert_eq!(active.start_value, -300.0);
    assert_eq!(active.end_value, 0.0);

    let has_focus_barrier = driver
        .harness
        .last_ir
        .as_ref()
        .unwrap()
        .nodes
        .values()
        .any(|node| {
            matches!(
                &node.op,
                fission_ir::Op::Semantics(semantics) if semantics.is_focus_barrier
            )
        });
    assert!(has_focus_barrier, "drawer overlay should trap focus");

    driver.tap_point(790.0, 10.0)?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert!(!state.drawer_open, "backdrop tap should close the drawer");
    driver.assert_text_not_visible("Drawer content");

    Ok(())
}
