use anyhow::Result;
use fission_core::ui::{Button, Text, Widget, ZStack};
use fission_core::{with_reducer, GlobalState, WidgetId};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{Drawer, DrawerMotion, DrawerSide, Modal, ModalMotion};

#[derive(Clone, Debug, Default)]
struct State {
    modal_open: bool,
    drawer_open: bool,
    background_presses: u32,
    modal_dismissals: u32,
    drawer_dismissals: u32,
}

impl GlobalState for State {}

#[fission_macros::fission_reducer(PressBackground)]
fn press_background(state: &mut State) {
    state.background_presses += 1;
}

#[fission_macros::fission_reducer(DismissModal)]
fn dismiss_modal(state: &mut State) {
    state.modal_open = false;
    state.modal_dismissals += 1;
}

#[fission_macros::fission_reducer(DismissDrawer)]
fn dismiss_drawer(state: &mut State) {
    state.drawer_open = false;
    state.drawer_dismissals += 1;
}

#[derive(Clone)]
struct ModalRoot;

#[derive(Clone)]
struct BackgroundButton;

impl From<BackgroundButton> for Widget {
    fn from(_button: BackgroundButton) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        Button {
            id: Some(WidgetId::explicit("overlay-exit.background")),
            child: Some(Text::new("Background").into()),
            on_press: Some(with_reducer!(ctx, PressBackground, press_background)),
            width: Some(800.0),
            height: Some(600.0),
            ..Default::default()
        }
        .semantics_identifier("background.action")
        .into()
    }
}

impl From<ModalRoot> for Widget {
    fn from(_root: ModalRoot) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        ZStack {
            children: vec![
                BackgroundButton.into(),
                Modal {
                    id: WidgetId::explicit("overlay-exit.modal"),
                    title: "Motion dialog".into(),
                    content: Text::new("Dialog body").into(),
                    is_open: view.state().modal_open,
                    on_dismiss: Some(with_reducer!(ctx, DismissModal, dismiss_modal)),
                    backdrop_semantics_identifier: Some("modal.backdrop".into()),
                    close_semantics_identifier: Some("modal.close".into()),
                    surface_semantics_identifier: Some("modal.surface".into()),
                    actions: vec![],
                    width: Some(360.0),
                    motion: Some(ModalMotion::Default),
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct DrawerRoot;

impl From<DrawerRoot> for Widget {
    fn from(_root: DrawerRoot) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        ZStack {
            children: vec![
                BackgroundButton.into(),
                Drawer {
                    id: WidgetId::explicit("overlay-exit.drawer"),
                    side: DrawerSide::Left,
                    is_open: view.state().drawer_open,
                    on_dismiss: Some(with_reducer!(ctx, DismissDrawer, dismiss_drawer)),
                    dismiss_semantics_identifier: Some("drawer.backdrop".into()),
                    content: Button {
                        id: Some(WidgetId::explicit("overlay-exit.drawer-action")),
                        child: Some(Text::new("Drawer body").into()),
                        ..Default::default()
                    }
                    .semantics_identifier("drawer.action")
                    .into(),
                    width: Some(300.0),
                    motion: Some(DrawerMotion::Default),
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn animated_modal_releases_pointer_input_as_soon_as_closing_begins() -> Result<()> {
    let harness = TestHarness::new_with_mock_measurer(State::default()).with_root_widget(ModalRoot);
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;

    driver.tap_point(760.0, 560.0)?;
    assert_modal_counts(&driver, 1, 0, false);

    driver
        .harness
        .runtime
        .get_app_state_mut::<State>()
        .expect("overlay test state")
        .modal_open = true;
    driver.pump()?;
    let close = driver
        .find_semantics_identifier("modal.close")
        .expect("modal close semantics");
    let close_point = (
        close.bounds.x() + close.bounds.width() / 2.0,
        close.bounds.y() + close.bounds.height() / 2.0,
    );
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(close.node_id),
        "the pointer-only backdrop must not become the dialog's keyboard entry target"
    );
    driver.tap_point(760.0, 560.0)?;
    assert_modal_counts(&driver, 1, 1, false);

    // The dialog and scrim are still painting their exit frame, but neither
    // may keep the full-window hit target, focus barrier, or accessibility
    // subtree alive.
    driver.assert_text_visible("Dialog body");
    assert!(driver.find_semantics_identifier("modal.surface").is_none());
    assert!(driver.find_semantics_identifier("modal.close").is_none());
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(
            driver
                .find_semantics_identifier("background.action")
                .expect("background semantics")
                .node_id
        )
    );
    assert!(fission_core::hit_test::topmost_focus_barrier(
        driver.harness.last_ir.as_ref().expect("modal exit IR")
    )
    .is_none());
    driver.tap_point(close_point.0, close_point.1)?;
    assert_modal_counts(&driver, 2, 1, false);

    driver.tick(220)?;
    driver.tap_point(760.0, 560.0)?;
    assert_modal_counts(&driver, 3, 1, false);
    Ok(())
}

#[test]
fn animated_drawer_releases_pointer_input_as_soon_as_closing_begins() -> Result<()> {
    let harness =
        TestHarness::new_with_mock_measurer(State::default()).with_root_widget(DrawerRoot);
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;

    driver.tap_point(760.0, 560.0)?;
    assert_drawer_counts(&driver, 1, 0, false);

    driver
        .harness
        .runtime
        .get_app_state_mut::<State>()
        .expect("overlay test state")
        .drawer_open = true;
    driver.pump()?;
    let action = driver
        .find_semantics_identifier("drawer.action")
        .expect("drawer action semantics");
    let action_point = (
        action.bounds.x() + action.bounds.width() / 2.0,
        action.bounds.y() + action.bounds.height() / 2.0,
    );
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(action.node_id),
        "the pointer-only backdrop must not become the drawer's keyboard entry target"
    );
    driver.tap_point(760.0, 560.0)?;
    assert_drawer_counts(&driver, 1, 1, false);

    driver.assert_text_visible("Drawer body");
    assert!(driver.find_semantics_identifier("drawer.action").is_none());
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(
            driver
                .find_semantics_identifier("background.action")
                .expect("background semantics")
                .node_id
        )
    );
    assert!(fission_core::hit_test::topmost_focus_barrier(
        driver.harness.last_ir.as_ref().expect("drawer exit IR")
    )
    .is_none());
    driver.tap_point(action_point.0, action_point.1)?;
    assert_drawer_counts(&driver, 2, 1, false);

    driver.tick(240)?;
    driver.tap_point(760.0, 560.0)?;
    assert_drawer_counts(&driver, 3, 1, false);
    Ok(())
}

fn assert_modal_counts(
    driver: &TestDriver<State>,
    background_presses: u32,
    dismissals: u32,
    is_open: bool,
) {
    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("overlay test state");
    assert_eq!(state.background_presses, background_presses);
    assert_eq!(state.modal_dismissals, dismissals);
    assert_eq!(state.modal_open, is_open);
}

fn assert_drawer_counts(
    driver: &TestDriver<State>,
    background_presses: u32,
    dismissals: u32,
    is_open: bool,
) {
    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("overlay test state");
    assert_eq!(state.background_presses, background_presses);
    assert_eq!(state.drawer_dismissals, dismissals);
    assert_eq!(state.drawer_open, is_open);
}
