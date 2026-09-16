use fission_test::{ir_texts, TestHarness};
use web_smoke::{CounterApp, CounterState};

/// Every empty field shows a hint, and the one-time-code tile carries a
/// visible caption instead of appearing as an unlabelled white box.
#[test]
fn empty_fields_show_hints_and_the_code_tile_is_labelled() {
    let mut harness = TestHarness::new(CounterState::default()).with_root_widget(CounterApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(1280.0, 860.0);
    harness.pump().expect("pump web smoke");
    let texts = ir_texts(harness.last_ir.as_ref().expect("ir"));

    for expected in [
        "Type to test spell checking",
        "Type anything",
        "Enter a password",
        "6-digit code",
        "Verification code",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected:?} missing from {texts:?}"
        );
    }
    for echo in ["Primary value:", "Secondary value:", "Verification value:"] {
        assert!(
            texts
                .iter()
                .any(|text| text.starts_with(&format!("{echo} (empty)"))),
            "{echo:?} should read \"(empty)\" rather than end bare: {texts:?}"
        );
    }
}
