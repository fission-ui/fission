use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{HStack, NumberInput, Wrap};

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
fn update_text(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    state.text_value = change.new_text.clone();
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

pub(crate) fn button() -> Vec<Widget> {
    widgets![ButtonDemo::new()]
}

fission_component! {
    ButtonDemo<GalleryState> |_demo, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let noop = with_reducer!(cx.build, Noop, noop);
    } =>
        Column(children: [
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.s,
            run_spacing: tokens.spacing.s,
            children: [
                Button(
                    variant: ButtonVariant::Filled,
                    child: Text("Primary"),
                    on_press: noop,
                )
                .semantics_identifier("gallery.button.filled"),
                Button(
                    variant: ButtonVariant::SecondaryGray,
                    child: Text("Secondary"),
                )
                .semantics_identifier("gallery.button.secondary"),
                Button(
                    variant: ButtonVariant::Outline,
                    child: Text("Outline"),
                )
                .semantics_identifier("gallery.button.outline"),
                Button(
                    variant: ButtonVariant::Ghost,
                    child: Text("Ghost"),
                )
                .semantics_identifier("gallery.button.ghost"),
                Button(
                    variant: ButtonVariant::Destructive,
                    child: Text("Delete"),
                )
                .semantics_identifier("gallery.button.destructive"),
                Button(
                    variant: ButtonVariant::LinkColor,
                    child: Text("Link"),
                )
                .semantics_identifier("gallery.button.link"),
                Button(
                    variant: ButtonVariant::Outline,
                    icon_content: ButtonIconContent::new(
                        Icon::svg(fission::icons::material::content::add::regular()),
                        "Add item",
                    ),
                )
                .semantics_identifier("gallery.button.icon"),
                Button(
                    variant: ButtonVariant::Filled,
                    child: Text("Disabled"),
                    disabled: true,
                )
                .semantics_identifier("gallery.button.disabled"),
            ],
        ),
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.s,
            run_spacing: tokens.spacing.s,
            children: [
                SizedButton(ComponentSize::Sm, "Small"),
                SizedButton(ComponentSize::Md, "Medium"),
                SizedButton(ComponentSize::Lg, "Large"),
                SizedButton(ComponentSize::Xl, "Extra large"),
            ],
        ),
    ])
}

fission_component! {
    SizedButton<()> { size: ComponentSize, label: &'static str }
    |button, _state, _cx| {} =>
        Button(size: button.size, child: Text(button.label))
}

pub(crate) fn text_input() -> Vec<Widget> {
    widgets![TextInputDemo::new()]
}

fission_component! {
    TextInputDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(children: [
        Container(TextInput(
            id: WidgetId::explicit("gallery.text_input"),
            semantics_identifier: "gallery.text_input".to_string(),
            value: state.text_value.clone(),
            placeholder: Some("Type something...".into()),
            on_input: with_reducer!(cx.build, UpdateText, update_text),
        ))
        .width_length(Length::clamp(
            Length::points(CONTROL_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(CONTROL_MAX_WIDTH),
        )),
        // Fields in a row share a top edge so their labels line up, even when
        // one grows to show an error.
        Row(
            gap: tokens.spacing.m,
            align_items: fission::op::AlignItems::Start,
            children: [
                Container(TextInput(
                    id: WidgetId::explicit("gallery.field.email"),
                    label: Some("Work email".into()),
                    placeholder: Some("ada@studio.co".into()),
                    helper_text: Some("We'll send the invite here.".into()),
                ))
                .width(CONTROL_MIN_WIDTH),
                Container(TextInput(
                    id: WidgetId::explicit("gallery.field.team"),
                    label: Some("Team name".into()),
                    value: "Platform team!",
                    error_text: Some("Use letters, numbers and spaces only.".into()),
                ))
                .width(CONTROL_MIN_WIDTH),
                Container(TextInput(
                    id: WidgetId::explicit("gallery.field.plan"),
                    label: Some("Plan".into()),
                    value: "Pro",
                    helper_text: Some("Managed by your admin.".into()),
                    enabled: false,
                ))
                .width(CONTROL_MIN_WIDTH),
            ],
        ),
    ])
}

pub(crate) fn checkbox() -> Vec<Widget> {
    widgets![CheckboxDemo::new()]
}

fission_component! {
    CheckboxDemo<GalleryState> |_demo, state, cx| {} =>
    HStack(
        spacing: None,
        children: [Checkbox(
            checked: state.checked,
            on_toggle: with_reducer!(cx.build, ToggleChecked, toggle_checked),
            label: Some("Accept terms and conditions".to_string()),
        )
        .semantics_identifier("gallery.checkbox")],
    )
}

pub(crate) fn switch() -> Vec<Widget> {
    widgets![SwitchDemo::new()]
}

fission_component! {
    SwitchDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    HStack(
        spacing: tokens.spacing.s,
        children: [
            Switch(
                checked: state.switch_on,
                on_toggle: with_reducer!(cx.build, ToggleSwitch, toggle_switch),
            )
            .semantics_identifier("gallery.switch"),
            Text("Airplane mode").color(tokens.colors.text_primary),
        ],
    )
}

pub(crate) fn slider() -> Vec<Widget> {
    widgets![SliderDemo::new()]
}

fission_component! {
    SliderDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    HStack(
        spacing: tokens.spacing.s,
        children: [
            Container(Slider(
                id: WidgetId::explicit("gallery.slider"),
                semantics_identifier: "gallery.slider".to_string(),
                value: state.slider_value,
                min: 0.0,
                max: 100.0,
                on_change: with_reducer!(cx.build, SetSlider(0.0), set_slider),
            ))
            .width_length(Length::clamp(
                Length::points(CONTROL_MIN_WIDTH),
                Length::percent(100.0),
                Length::points(SLIDER_MAX_WIDTH),
            )),
            Text(format!("{:.0}", state.slider_value)).color(tokens.colors.text_secondary),
        ],
    )
}

pub(crate) fn number_input() -> Vec<Widget> {
    widgets![NumberInputDemo::new()]
}

fission_component! {
    NumberInputDemo<GalleryState> |_demo, state, cx| {} =>
    HStack(
        spacing: None,
        children: [NumberInput(
            value: state.number_value,
            step: 1.0,
            on_increment: with_reducer!(cx.build, IncrementNumber, increment_number),
            on_decrement: with_reducer!(cx.build, DecrementNumber, decrement_number),
        )],
    )
}
