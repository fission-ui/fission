use fission::prelude::*;

#[test]
fn range_slider_and_its_contextual_input_types_are_in_the_prelude() {
    let _widget: Widget = RangeSlider {
        start: 10.0,
        end: 90.0,
        min: 0.0,
        max: 100.0,
        ..Default::default()
    }
    .semantics_identifier("filters.price")
    .step(5.0)
    .into();

    let _: Option<RangeSliderChanged> = None;
    let _: RangeSliderThumb = RangeSliderThumb::Start;
    let _: RangeSliderChangeSource = RangeSliderChangeSource::Keyboard;
}
