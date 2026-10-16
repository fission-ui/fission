use crate::app::ShowcaseApp;
use crate::i18n::create_env;
use crate::state::ShowcaseState;
use fission::layout::{LayoutPoint, LayoutSize};
use fission::prelude::{
    InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerKind, WidgetId,
};
use fission_test::{SemanticMatch, TestDriver, TestHarness};

fn driver(path: &str) -> TestDriver<ShowcaseState> {
    let state = ShowcaseState {
        current_path: path.into(),
        ..ShowcaseState::default()
    };
    let mut harness = TestHarness::new(state).with_root_widget(ShowcaseApp);
    harness.env = create_env().expect("showcase environment");
    harness.env.viewport_size = LayoutSize::new(1440.0, 960.0);
    let mut driver = TestDriver::new(harness);
    driver.pump().expect("first showcase frame");
    driver.pump().expect("settled showcase frame");
    driver
}

/// Real shells rebuild between pointer down and pointer up. Keeping the two
/// phases separate catches retained-state and action-registration bugs that a
/// synthetic down/up pair in one frame cannot expose.
fn click_with_intermediate_frame(driver: &mut TestDriver<ShowcaseState>, target: SemanticMatch) {
    let point = LayoutPoint::new(
        target.bounds.x() + target.bounds.width() / 2.0,
        target.bounds.y() + target.bounds.height() / 2.0,
    );
    click_point_with_intermediate_frame(driver, point);
}

fn click_point_with_intermediate_frame(driver: &mut TestDriver<ShowcaseState>, point: LayoutPoint) {
    driver
        .harness
        .send_event(InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: PointerKind::Touch,
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }))
        .expect("pointer down");
    driver.pump().expect("pressed frame");
    driver
        .harness
        .send_event(InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: PointerKind::Touch,
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }))
        .expect("pointer up");
    driver.pump().expect("released frame");
}

fn click_identifier(driver: &mut TestDriver<ShowcaseState>, identifier: &str) {
    let target = driver
        .find_semantics_identifier(identifier)
        .unwrap_or_else(|| panic!("missing {identifier}"));
    click_with_intermediate_frame(driver, target);
}

fn click_identifier_at(
    driver: &mut TestDriver<ShowcaseState>,
    identifier: &str,
    relative_x: f32,
    relative_y: f32,
) {
    let target = driver
        .find_semantics_identifier(identifier)
        .unwrap_or_else(|| panic!("missing {identifier}"));
    click_point_with_intermediate_frame(
        driver,
        LayoutPoint::new(
            target.bounds.x() + target.bounds.width() * relative_x,
            target.bounds.y() + target.bounds.height() * relative_y,
        ),
    );
}

fn click_text(driver: &mut TestDriver<ShowcaseState>, text: &str) {
    let target = driver
        .find_text(text)
        .unwrap_or_else(|| panic!("missing visible text {text}"));
    click_point_with_intermediate_frame(
        driver,
        LayoutPoint::new(
            target.bounds.x() + target.bounds.width() / 2.0,
            target.bounds.y() + target.bounds.height() / 2.0,
        ),
    );
}

#[test]
fn mounted_examples_keep_focus_dispatch_actions_and_forward_portals() {
    let mut driver = driver("/examples/text-lab");

    click_identifier(&mut driver, "text-lab.single-line");
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(WidgetId::explicit("text_lab_single_line")),
        "the mounted field must retain focus across the pointer-down rebuild"
    );

    driver
        .harness
        .send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Tab,
            modifiers: 0,
        }))
        .expect("Tab traversal");
    driver.pump().expect("frame after Tab");
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(WidgetId::explicit("text_lab_multiline")),
        "Tab should move from the single-line field to the multiline field"
    );

    driver
        .type_text("Mounted text")
        .expect("type multiline text");
    assert_eq!(
        crate::mounted_example::mounted_state::<text_lab_example::TextLabState>(
            "showcase.example.text-lab"
        )
        .multiline,
        "Mounted text",
        "multiline input must update the mounted example's authoritative state"
    );
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(WidgetId::explicit("text_lab_multiline")),
        "typing and rebuilding must not discard multiline focus"
    );

    click_identifier(&mut driver, "text-lab.open-modal");
    driver
        .tick(500)
        .expect("finish the mounted modal entrance motion");
    let modal = driver
        .find_semantics_identifier("text-lab.modal")
        .expect("a mounted example action must rebuild the host and forward its modal portal");
    assert!(
        modal.bounds.width() > 0.0 && modal.bounds.height() > 0.0,
        "the forwarded modal must keep non-zero layout bounds"
    );

    click_text(&mut driver, "Cancel");
    assert!(
        !crate::mounted_example::mounted_state::<text_lab_example::TextLabState>(
            "showcase.example.text-lab"
        )
        .show_modal,
        "the mounted modal close action must update its owning state"
    );
    driver.tick(500).expect("finish any modal exit motion");
    assert!(
        driver.find_semantics_identifier("text-lab.modal").is_none(),
        "the mounted modal action must close the forwarded portal"
    );

    click_identifier(&mut driver, "showcase.example.animation-gallery");
    assert_eq!(
        driver
            .harness
            .runtime
            .get_global_state::<ShowcaseState>()
            .expect("showcase state")
            .current_path,
        "/examples/animation-gallery",
        "catalogue navigation must dispatch after a visible pressed frame"
    );

    // A control must be actionable across its painted body, not only at one
    // fortunate centre coordinate. Alternate previews so every press causes an
    // observable route change.
    for relative_x in [0.2, 0.5, 0.8] {
        click_identifier_at(&mut driver, "showcase.example.text-lab", relative_x, 0.5);
        assert_eq!(
            driver
                .harness
                .runtime
                .get_global_state::<ShowcaseState>()
                .expect("showcase state")
                .current_path,
            "/examples/text-lab"
        );
        click_identifier_at(
            &mut driver,
            "showcase.example.animation-gallery",
            relative_x,
            0.5,
        );
        assert_eq!(
            driver
                .harness
                .runtime
                .get_global_state::<ShowcaseState>()
                .expect("showcase state")
                .current_path,
            "/examples/animation-gallery"
        );
    }

    // The embedded animation gallery starts on its overview. Open its Modal
    // route, then exercise the actual modal rather than merely its navigation.
    click_identifier(&mut driver, "animation-gallery.nav.widgets.modal");
    click_identifier(&mut driver, "animation-gallery.action.open-real-modal");
    assert!(
        driver
            .find_semantics_identifier("gallery.real.modal.surface")
            .is_some(),
        "animation gallery controls must dispatch and mount their modal portal"
    );

    click_identifier(&mut driver, "gallery.real.modal.cancel");
    driver
        .tick(500)
        .expect("finish animation modal exit motion");
    assert!(
        driver
            .find_semantics_identifier("gallery.real.modal.surface")
            .is_none(),
        "the mounted animation modal must close"
    );

    click_identifier(&mut driver, "animation-gallery.nav.widgets.popover");
    click_identifier(&mut driver, "gallery.real.popover.trigger");
    assert!(
        driver.find_text("Invite teammate").is_some(),
        "the popover trigger must own an action that opens its anchored portal"
    );
    click_identifier(&mut driver, "gallery.real.popover.close");
    driver
        .tick(500)
        .expect("finish animation popover exit motion");
    assert!(
        driver.find_text("Invite teammate").is_none(),
        "the mounted animation popover must close"
    );
}
