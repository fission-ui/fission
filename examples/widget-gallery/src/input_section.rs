use crate::gallery_section::GallerySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{HStack, NumberInput};

const CONTROL_MIN_WIDTH: f32 = 220.0;
const CONTROL_MAX_WIDTH: f32 = 420.0;
const SLIDER_MAX_WIDTH: f32 = 280.0;

#[fission_reducer(SetSlider, no_eq)]
fn set_slider(state: &mut GalleryState, value: f32) {
    state.slider_value = value;
}

#[fission_reducer(ToggleChecked)]
fn toggle_checked(state: &mut GalleryState) {
    state.checked = !state.checked;
}

#[fission_reducer(ToggleSwitch)]
fn toggle_switch(state: &mut GalleryState) {
    state.switch_on = !state.switch_on;
}

#[fission_reducer(UpdateText)]
fn update_text(state: &mut GalleryState, value: String) {
    state.text_value = value;
}

#[fission_reducer(IncrementNumber)]
fn increment_number(state: &mut GalleryState) {
    state.number_value += 1.0;
}

#[fission_reducer(DecrementNumber)]
fn decrement_number(state: &mut GalleryState) {
    state.number_value -= 1.0;
}

#[fission_reducer(Noop)]
fn noop(_state: &mut GalleryState) {}

pub(crate) struct InputSection;

impl From<InputSection> for Widget {
    fn from(_section: InputSection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;

        let noop = with_reducer!(ctx, Noop, noop);
        let update_text = with_reducer!(ctx, UpdateText(String::new()), update_text);
        let toggle_checked = with_reducer!(ctx, ToggleChecked, toggle_checked);
        let toggle_switch = with_reducer!(ctx, ToggleSwitch, toggle_switch);
        let set_slider = with_reducer!(ctx, SetSlider(0.0), set_slider);
        let increment_number = with_reducer!(ctx, IncrementNumber, increment_number);
        let decrement_number = with_reducer!(ctx, DecrementNumber, decrement_number);

        GallerySection::new(
            "Input",
            widgets![
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Text::new("Filled").into()),
                            on_press: Some(noop),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.button.filled"),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Outline").into()),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.button.outline"),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Text::new("Ghost").into()),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.button.ghost"),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Text::new("Disabled").into()),
                            disabled: true,
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.button.disabled"),
                    ],
                },
                Container::new(TextInput {
                    id: Some(WidgetId::explicit("gallery.text_input")),
                    semantics_identifier: Some("gallery.text_input".into()),
                    value: state.text_value.clone(),
                    placeholder: Some("Type something...".into()),
                    on_change: Some(update_text),
                    ..Default::default()
                })
                .width_length(Length::clamp(
                    Length::points(CONTROL_MIN_WIDTH),
                    Length::percent(100.0),
                    Length::points(CONTROL_MAX_WIDTH),
                )),
                HStack {
                    spacing: Some(tokens.spacing.m),
                    children: widgets![
                        Checkbox {
                            checked: state.checked,
                            on_toggle: Some(toggle_checked),
                            label: Some("Check me".into()),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.checkbox"),
                        Switch {
                            checked: state.switch_on,
                            on_toggle: Some(toggle_switch),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.switch"),
                    ],
                },
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Text::new("Slider:").color(tokens.colors.text_primary),
                        Container::new(Slider {
                            id: Some(WidgetId::explicit("gallery.slider")),
                            semantics_identifier: Some("gallery.slider".into()),
                            value: state.slider_value,
                            min: 0.0,
                            max: 100.0,
                            on_change: Some(set_slider),
                            ..Default::default()
                        })
                        .width_length(Length::clamp(
                            Length::points(CONTROL_MIN_WIDTH),
                            Length::percent(100.0),
                            Length::points(SLIDER_MAX_WIDTH),
                        )),
                        Text::new(format!("{:.0}", state.slider_value))
                            .color(tokens.colors.text_secondary),
                    ],
                },
                NumberInput {
                    value: state.number_value,
                    step: 1.0,
                    on_increment: Some(increment_number),
                    on_decrement: Some(decrement_number),
                    ..Default::default()
                },
            ],
        )
        .into()
    }
}
