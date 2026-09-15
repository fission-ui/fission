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
    let (ctx, view) = fission::build::current::<GalleryState>();
    let tokens = &view.env().theme.tokens;
    let noop = with_reducer!(ctx, Noop, noop);
    let sized = |size: ComponentSize, label: &str| -> Widget {
        Button {
            size,
            child: Some(Text::new(label).into()),
            ..Default::default()
        }
        .into()
    };

    widgets![
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.s),
            run_spacing: Some(tokens.spacing.s),
            children: widgets![
                Button {
                    variant: ButtonVariant::Filled,
                    child: Some(Text::new("Primary").into()),
                    on_press: Some(noop),
                    ..Default::default()
                }
                .semantics_identifier("gallery.button.filled"),
                Button {
                    variant: ButtonVariant::SecondaryGray,
                    child: Some(Text::new("Secondary").into()),
                    ..Default::default()
                }
                .semantics_identifier("gallery.button.secondary"),
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
                    variant: ButtonVariant::Destructive,
                    child: Some(Text::new("Delete").into()),
                    ..Default::default()
                }
                .semantics_identifier("gallery.button.destructive"),
                Button {
                    variant: ButtonVariant::LinkColor,
                    child: Some(Text::new("Link").into()),
                    ..Default::default()
                }
                .semantics_identifier("gallery.button.link"),
                Button {
                    variant: ButtonVariant::Outline,
                    icon_content: Some(ButtonIconContent::new(
                        Icon::svg(fission::icons::material::content::add::regular()),
                        "Add item",
                    )),
                    ..Default::default()
                }
                .semantics_identifier("gallery.button.icon"),
                Button {
                    variant: ButtonVariant::Filled,
                    child: Some(Text::new("Disabled").into()),
                    disabled: true,
                    ..Default::default()
                }
                .semantics_identifier("gallery.button.disabled"),
            ],
        },
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.s),
            run_spacing: Some(tokens.spacing.s),
            children: vec![
                sized(ComponentSize::Sm, "Small"),
                sized(ComponentSize::Md, "Medium"),
                sized(ComponentSize::Lg, "Large"),
                sized(ComponentSize::Xl, "Extra large"),
            ],
        },
    ]
}

pub(crate) fn text_input() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let update_text = with_reducer!(ctx, UpdateText, update_text);

    widgets![
        Container::new(TextInput {
            id: Some(WidgetId::explicit("gallery.text_input")),
            semantics_identifier: Some("gallery.text_input".into()),
            value: state.text_value.clone(),
            placeholder: Some("Type something...".into()),
            on_input: Some(update_text),
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(CONTROL_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(CONTROL_MAX_WIDTH),
        )),
        // Fields in a row share a top edge so their labels line up, even when
        // one grows to show an error.
        Row {
            gap: Some(tokens.spacing.m),
            align_items: fission::op::AlignItems::Start,
            children: widgets![
                Container::new(TextInput {
                    id: Some(WidgetId::explicit("gallery.field.email")),
                    label: Some("Work email".into()),
                    placeholder: Some("ada@studio.co".into()),
                    helper_text: Some("We'll send the invite here.".into()),
                    ..Default::default()
                })
                .width(CONTROL_MIN_WIDTH),
                Container::new(TextInput {
                    id: Some(WidgetId::explicit("gallery.field.team")),
                    label: Some("Team name".into()),
                    value: "Platform team!".into(),
                    error_text: Some("Use letters, numbers and spaces only.".into()),
                    ..Default::default()
                })
                .width(CONTROL_MIN_WIDTH),
                Container::new(TextInput {
                    id: Some(WidgetId::explicit("gallery.field.plan")),
                    label: Some("Plan".into()),
                    value: "Pro".into(),
                    helper_text: Some("Managed by your admin.".into()),
                    enabled: false,
                    ..Default::default()
                })
                .width(CONTROL_MIN_WIDTH),
            ],
            ..Default::default()
        },
    ]
}

pub(crate) fn checkbox() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let toggle_checked = with_reducer!(ctx, ToggleChecked, toggle_checked);
    widgets![HStack {
        spacing: None,
        children: widgets![Checkbox {
            checked: state.checked,
            on_toggle: Some(toggle_checked),
            label: Some("Accept terms and conditions".into()),
            ..Default::default()
        }
        .semantics_identifier("gallery.checkbox")],
    }]
}

pub(crate) fn switch() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let toggle_switch = with_reducer!(ctx, ToggleSwitch, toggle_switch);
    widgets![HStack {
        spacing: Some(tokens.spacing.s),
        children: widgets![
            Switch {
                checked: state.switch_on,
                on_toggle: Some(toggle_switch),
                ..Default::default()
            }
            .semantics_identifier("gallery.switch"),
            Text::new("Airplane mode").color(tokens.colors.text_primary),
        ],
    }]
}

pub(crate) fn slider() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let set_slider = with_reducer!(ctx, SetSlider(0.0), set_slider);
    widgets![HStack {
        spacing: Some(tokens.spacing.s),
        children: widgets![
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
            Text::new(format!("{:.0}", state.slider_value)).color(tokens.colors.text_secondary),
        ],
    }]
}

pub(crate) fn number_input() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let increment_number = with_reducer!(ctx, IncrementNumber, increment_number);
    let decrement_number = with_reducer!(ctx, DecrementNumber, decrement_number);
    widgets![HStack {
        spacing: None,
        children: widgets![NumberInput {
            value: state.number_value,
            step: 1.0,
            on_increment: Some(increment_number),
            on_decrement: Some(decrement_number),
            ..Default::default()
        }],
    }]
}
