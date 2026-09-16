use fission_test::{ir_texts, TestHarness};
use text_lab::{TextLabApp, TextLabState};

fn texts(state: TextLabState) -> Vec<String> {
    let mut harness = TestHarness::new(state).with_root_widget(TextLabApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(1280.0, 860.0);
    harness.pump().expect("pump text lab");
    ir_texts(harness.last_ir.as_ref().expect("ir"))
}

/// The empty combobox tells people what to type.
#[test]
fn empty_combobox_shows_its_placeholder() {
    let texts = texts(TextLabState::default());
    assert!(
        texts.iter().any(|text| text == "Start typing an address"),
        "combobox placeholder missing from {texts:?}"
    );
}

/// The status line stays hidden until an event gives it a value, so it never
/// reads as a bare "Status:" label.
#[test]
fn status_line_appears_only_with_a_value() {
    let idle = texts(TextLabState::default());
    assert!(
        !idle.iter().any(|text| text.trim_start().starts_with("Status:")),
        "idle harness shows a status line: {idle:?}"
    );

    let recorded = texts(TextLabState {
        status: "Menu action: archive_selected".into(),
        ..Default::default()
    });
    assert!(
        recorded
            .iter()
            .any(|text| text == "Status: Menu action: archive_selected"),
        "recorded status missing from {recorded:?}"
    );
}
