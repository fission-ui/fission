use crate::gallery_section::GallerySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{ColourHsva, ColourPicker, ColourPickerVariant, HStack, Wrap};
use std::sync::Arc;

const COLOUR_PREVIEW_SIZE: f32 = 28.0;
const BORDER_WIDTH: f32 = 1.0;

#[fission_reducer(SetGalleryColour, no_eq)]
fn set_gallery_colour(state: &mut GalleryState, colour: Color) {
    state.colour_value = colour;
}

#[fission_reducer(SetGalleryColourHue, no_eq)]
fn set_gallery_colour_hue(state: &mut GalleryState, hue: f32) {
    let mut hsva = ColourHsva::from_color(state.colour_value);
    hsva.hue = hue;
    if hsva.saturation <= f32::EPSILON {
        hsva.saturation = 1.0;
    }
    if hsva.value <= f32::EPSILON {
        hsva.value = 1.0;
    }
    state.colour_value = hsva.to_color();
}

#[fission_reducer(SetGalleryColourSaturation, no_eq)]
fn set_gallery_colour_saturation(state: &mut GalleryState, saturation: f32) {
    let mut hsva = ColourHsva::from_color(state.colour_value);
    hsva.saturation = saturation;
    state.colour_value = hsva.to_color();
}

#[fission_reducer(SetGalleryColourValue, no_eq)]
fn set_gallery_colour_value(state: &mut GalleryState, value: f32) {
    let mut hsva = ColourHsva::from_color(state.colour_value);
    hsva.value = value;
    state.colour_value = hsva.to_color();
}

#[fission_reducer(SetGalleryColourAlpha, no_eq)]
fn set_gallery_colour_alpha(state: &mut GalleryState, alpha: f32) {
    let mut hsva = ColourHsva::from_color(state.colour_value);
    hsva.alpha = alpha;
    state.colour_value = hsva.to_color();
}

#[fission_reducer(SetGalleryColourHex)]
fn set_gallery_colour_hex(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    if let Some(colour) = parse_gallery_hex(&change.new_text) {
        state.colour_value = colour;
    }
}

#[fission_reducer(SetColourVariant)]
fn set_colour_variant(state: &mut GalleryState, variant: usize) {
    state.colour_variant = variant;
}

pub(crate) struct ColourPickerSection;

impl From<ColourPickerSection> for Widget {
    fn from(_section: ColourPickerSection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let variants = colour_variants();
        let active_variant = variants
            .get(state.colour_variant)
            .map(|(_, variant)| *variant)
            .unwrap_or_default();
        let colour_change = Arc::new({
            let action = with_reducer!(
                ctx,
                SetGalleryColour(state.colour_value),
                set_gallery_colour
            );
            move |colour: Color| ActionEnvelope {
                id: action.id,
                payload: serde_json::to_vec(&colour).unwrap(),
            }
        });

        GallerySection::new(
            "Colour Picker",
            widgets![
                Text::new(
                    "Switch between the built-in picker variants, then edit the same controlled colour value.",
                )
                .color(tokens.colors.text_secondary),
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Container::new(Text::new(""))
                            .size(COLOUR_PREVIEW_SIZE, COLOUR_PREVIEW_SIZE)
                            .bg(state.colour_value)
                            .border(tokens.colors.border, BORDER_WIDTH)
                            .border_radius(tokens.radii.small),
                        Text::new(format!(
                            "Current {}",
                            gallery_hex_string(state.colour_value)
                        ))
                        .color(tokens.colors.text_secondary),
                    ],
                    ..Default::default()
                },
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.xs),
                    children: variants
                        .iter()
                        .enumerate()
                        .map(|(index, (label, _))| {
                            Button {
                                variant: if index == state.colour_variant {
                                    ButtonVariant::Filled
                                } else {
                                    ButtonVariant::Outline
                                },
                                child: Some(Text::new(*label).into()),
                                on_press: Some(with_reducer!(
                                    ctx,
                                    SetColourVariant(index),
                                    set_colour_variant
                                )),
                                ..Default::default()
                            }
                            .semantics_identifier(format!("gallery.colour.variant.{label}"))
                            .into()
                        })
                        .collect(),
                },
                ColourPicker {
                    id: Some(WidgetId::explicit("gallery_colour_picker")),
                    semantics_identifier: Some("gallery.colour".into()),
                    value: state.colour_value,
                    variant: active_variant,
                    recent: recent_colours(),
                    on_change: Some(colour_change),
                    on_hue_change: Some(with_reducer!(
                        ctx,
                        SetGalleryColourHue(0.0),
                        set_gallery_colour_hue
                    )),
                    on_saturation_change: Some(with_reducer!(
                        ctx,
                        SetGalleryColourSaturation(0.0),
                        set_gallery_colour_saturation
                    )),
                    on_value_change: Some(with_reducer!(
                        ctx,
                        SetGalleryColourValue(0.0),
                        set_gallery_colour_value
                    )),
                    on_alpha_change: Some(with_reducer!(
                        ctx,
                        SetGalleryColourAlpha(1.0),
                        set_gallery_colour_alpha
                    )),
                    on_hex_input: Some(with_reducer!(
                        ctx,
                        SetGalleryColourHex,
                        set_gallery_colour_hex
                    )),
                    ..Default::default()
                },
            ],
        )
        .into()
    }
}

fn colour_variants() -> &'static [(&'static str, ColourPickerVariant)] {
    &[
        ("Chrome", ColourPickerVariant::Chrome),
        ("Sketch", ColourPickerVariant::Sketch),
        ("Photoshop", ColourPickerVariant::Photoshop),
        ("Compact", ColourPickerVariant::Compact),
        ("Circle", ColourPickerVariant::Circle),
        ("GitHub", ColourPickerVariant::Github),
        ("Twitter", ColourPickerVariant::Twitter),
        ("Material", ColourPickerVariant::Material),
        ("Slider", ColourPickerVariant::Slider),
        ("Swatches", ColourPickerVariant::Swatches),
        ("Block", ColourPickerVariant::Block),
        ("Hue", ColourPickerVariant::Hue),
        ("Alpha", ColourPickerVariant::Alpha),
    ]
}

fn recent_colours() -> Vec<Color> {
    vec![
        Color {
            r: 16,
            g: 185,
            b: 129,
            a: 255,
        },
        Color {
            r: 244,
            g: 63,
            b: 94,
            a: 255,
        },
        Color {
            r: 245,
            g: 158,
            b: 11,
            a: 255,
        },
    ]
}

fn parse_gallery_hex(value: &str) -> Option<Color> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }

    Some(Color {
        r: u8::from_str_radix(&hex[0..2], 16).ok()?,
        g: u8::from_str_radix(&hex[2..4], 16).ok()?,
        b: u8::from_str_radix(&hex[4..6], 16).ok()?,
        a: if hex.len() == 8 {
            u8::from_str_radix(&hex[6..8], 16).ok()?
        } else {
            255
        },
    })
}

fn gallery_hex_string(colour: Color) -> String {
    if colour.a == 255 {
        format!("#{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b)
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            colour.r, colour.g, colour.b, colour.a
        )
    }
}
