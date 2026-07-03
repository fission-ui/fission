use anyhow::Result;
use fission_core::ui::{Node, Text};
use fission_core::{
    ActionEnvelope, ActionId, AnimationPropertyId, AppState, BuildCtx, View, Widget, WidgetNodeId,
};
use fission_ir::semantics::{Role, TextInputType};
use fission_render::DisplayOp;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{CircularProgress, DatePicker, Drawer, DrawerSide, NumberInput};
use std::f32::consts::PI;
use std::sync::Arc;

const NUMBER_CHANGED_ID: ActionId = ActionId::from_u128(0xF151_0001);
const DATE_NAVIGATED_ID: ActionId = ActionId::from_u128(0xF151_0002);
const DRAWER_DISMISSED_ID: ActionId = ActionId::from_u128(0xF151_0003);
const DATE_SELECTED_ID: ActionId = ActionId::from_u128(0xF151_0004);

#[derive(Debug, Clone)]
struct State {
    number: f32,
    date_year: i32,
    date_month: u32,
    selected_date: Option<String>,
    drawer_open: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            number: 0.0,
            date_year: 2026,
            date_month: 5,
            selected_date: None,
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
fn number_input_ignores_invalid_intermediate_float() -> Result<()> {
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

    let input = driver
        .find_role(Role::TextInput)
        .into_iter()
        .next()
        .expect("NumberInput text field");
    driver.tap_point(
        input.bounds.x() + input.bounds.width() / 2.0,
        input.bounds.y() + input.bounds.height() / 2.0,
    )?;
    driver.type_text("-")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(
        state.number, 0.0,
        "invalid intermediate numeric text must not dispatch a parsed value"
    );

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

    driver.tap_text("<")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!((state.date_year, state.date_month), (2026, 5));
    driver.assert_text_visible("May 2026");

    Ok(())
}

#[test]
fn date_picker_navigation_wraps_year_and_selection_dispatches_date() -> Result<()> {
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
            ctx.registry.register_raw_action(
                DATE_SELECTED_ID,
                |state, envelope, _target, _effects, _input| {
                    state.selected_date =
                        Some(serde_json::from_slice::<String>(&envelope.payload)?);
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
                on_change: Some(Arc::new(|date| ActionEnvelope {
                    id: DATE_SELECTED_ID,
                    payload: serde_json::to_vec(&date.to_string()).unwrap(),
                })),
                on_toggle: None,
                on_close: None,
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State {
        date_year: 2026,
        date_month: 12,
        ..State::default()
    })
    .with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;
    driver.assert_text_visible("December 2026");

    driver.tap_text(">")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!((state.date_year, state.date_month), (2027, 1));
    driver.assert_text_visible("January 2027");

    driver.tap_text("15")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.selected_date.as_deref(), Some("2027-01-15"));

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

    let backdrop_anim_id = WidgetNodeId::from_u128(drawer_id.as_u128() ^ 0xBACD_u128);
    let backdrop = driver
        .harness
        .runtime
        .runtime_state
        .animation
        .active
        .get(&(backdrop_anim_id, AnimationPropertyId::Opacity))
        .expect("drawer backdrop opacity animation");
    assert_eq!(backdrop.start_value, 0.0);
    assert_eq!(backdrop.end_value, 1.0);

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

    driver.tick(300)?;
    driver.tap_text("Drawer content")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert!(
        state.drawer_open,
        "tapping inside the drawer panel must not trigger backdrop dismissal"
    );

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

#[test]
fn right_drawer_slides_from_clamped_offscreen_edge() -> Result<()> {
    let drawer_id = WidgetNodeId::explicit("right_drawer");

    struct Root {
        drawer_id: WidgetNodeId,
    }

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            Drawer {
                id: self.drawer_id,
                side: DrawerSide::Right,
                is_open: view.state.drawer_open,
                on_dismiss: None,
                content: Box::new(Text::new("Right drawer content").into_node()),
                width: Some(500.0),
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { drawer_id });
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(360.0, 640.0);
    driver.pump()?;

    let slide_anim_id = WidgetNodeId::from_u128(drawer_id.as_u128() ^ 0xD00D_u128);
    let active = driver
        .harness
        .runtime
        .runtime_state
        .animation
        .active
        .get(&(slide_anim_id, AnimationPropertyId::TranslateX))
        .expect("right drawer slide animation");
    assert_eq!(active.start_value, 336.0);
    assert_eq!(active.end_value, 0.0);

    Ok(())
}

#[test]
fn circular_progress_indeterminate_registers_repeating_rotation() -> Result<()> {
    let progress_id = WidgetNodeId::explicit("loading_spinner");

    struct Root {
        progress_id: WidgetNodeId,
    }

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            CircularProgress {
                id: Some(self.progress_id),
                value: None,
                ..Default::default()
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { progress_id });
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    let key = (progress_id, AnimationPropertyId::Rotation);
    let active = driver
        .harness
        .runtime
        .runtime_state
        .animation
        .active
        .get(&key)
        .expect("indeterminate progress rotation animation");
    assert_eq!(active.start_value, 0.0);
    assert!((active.end_value - 2.0 * PI).abs() < 0.001);
    assert!(active.repeat);
    assert_eq!(active.frame_interval_ms, Some(16));

    driver.tick(250)?;

    let current = driver
        .harness
        .runtime
        .runtime_state
        .animation
        .values
        .get(&key)
        .copied()
        .expect("animated rotation value");
    assert!(
        current > 0.0 && current < 2.0 * PI,
        "rotation should advance after ticking, got {current}"
    );
    let has_transform = driver
        .harness
        .get_last_display_list()
        .map(|display_list| {
            display_list
                .ops
                .iter()
                .any(|op| matches!(op, DisplayOp::Transform(_)))
        })
        .unwrap_or(false);
    assert!(
        has_transform,
        "animated circular progress should render through a composite transform"
    );

    Ok(())
}

#[test]
fn circular_progress_indeterminate_without_id_renders_static_indicator() -> Result<()> {
    struct Root;

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            CircularProgress {
                id: None,
                value: None,
                ..Default::default()
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .animation
            .active
            .is_empty(),
        "indeterminate progress without an id should not register an animation"
    );
    let has_transform = driver
        .harness
        .get_last_display_list()
        .map(|display_list| {
            display_list
                .ops
                .iter()
                .any(|op| matches!(op, DisplayOp::Transform(_)))
        })
        .unwrap_or(false);
    assert!(
        !has_transform,
        "indeterminate progress without an id should render the static arc directly"
    );

    Ok(())
}

#[test]
fn circular_progress_determinate_does_not_register_rotation() -> Result<()> {
    let progress_id = WidgetNodeId::explicit("static_progress");

    struct Root {
        progress_id: WidgetNodeId,
    }

    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            CircularProgress {
                id: Some(self.progress_id),
                value: Some(0.5),
                ..Default::default()
            }
            .build(ctx, view)
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { progress_id });
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    assert!(
        !driver
            .harness
            .runtime
            .runtime_state
            .animation
            .active
            .contains_key(&(progress_id, AnimationPropertyId::Rotation)),
        "determinate progress should not spin"
    );

    Ok(())
}
