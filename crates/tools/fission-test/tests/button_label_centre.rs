//! A button's label sits in the vertical middle of the button.

use anyhow::Result;
use fission_core::ui::{Button, ButtonVariant, Column, Text, Widget};
use fission_core::GlobalState;
use fission_test::{TestDriver, TestHarness};

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_: Root) -> Self {
        Column {
            children: vec![Button {
                variant: ButtonVariant::Outline,
                child: Some(Text::new("Chrome").into()),
                ..Default::default()
            }
            .semantics_identifier("button-centre.button")
            .into()],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn outline_button_label_is_vertically_centred() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Root));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(400.0, 200.0);
    driver.pump()?;
    let button = driver
        .find_semantics_identifier("button-centre.button")
        .expect("button semantics")
        .bounds;
    let label = driver.find_text("Chrome").expect("label text").bounds;
    let button_centre = button.y() + button.height() / 2.0;
    let label_centre = label.y() + label.height() / 2.0;
    assert!(
        (button_centre - label_centre).abs() <= 1.5,
        "label centre {label_centre} should match button centre {button_centre}; button {button:?}, label {label:?}"
    );
    Ok(())
}
