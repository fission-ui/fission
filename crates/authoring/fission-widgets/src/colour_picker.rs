use crate::Wrap;
use fission_core::ui::{
    Button, ButtonVariant, Column, Container, Row, Slider, Text, TextInput, Widget,
};
use fission_core::ActionEnvelope;
use fission_ir::op::{AlignItems, Color, Fill, FlexDirection, JustifyContent};
use fission_ir::{Role, Semantics, WidgetId};
use std::sync::Arc;

/// Built-in colour picker layouts inspired by common design-tool pickers.
///
/// Each variant is the same controlled widget rendered with a different density
/// and control mix. Use [`ColourPicker::variant`] to switch between compact
/// swatches, editor-style controls, social preset palettes, or individual hue
/// and alpha controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourPickerVariant {
    /// Chrome DevTools style: preview, saturation/value controls, hue, alpha and inputs.
    Chrome,
    /// Sketch style: editor controls with a denser preset strip.
    Sketch,
    /// Photoshop style: large preview plus detailed numeric fields.
    Photoshop,
    /// Compact style: only a tight palette grid.
    Compact,
    /// Circle style: round swatches for quick theme selection.
    Circle,
    /// GitHub style: small preset palette suitable for popovers and issue labels.
    Github,
    /// Twitter style: friendly social palette with a large preview.
    Twitter,
    /// Material style: Material Design colour families.
    Material,
    /// Slider style: preview plus hue, saturation, value and optional alpha sliders.
    Slider,
    /// Swatches style: large grouped colour families.
    Swatches,
    /// Block style: prominent preview block with editable hex and swatches.
    Block,
    /// Hue-only control.
    Hue,
    /// Alpha-only control.
    Alpha,
}

impl Default for ColourPickerVariant {
    fn default() -> Self {
        Self::Chrome
    }
}

/// HSV representation used by [`ColourPicker`] slider helpers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColourHsva {
    /// Hue in degrees, clamped to `0..=360`.
    pub hue: f32,
    /// Saturation in `0..=1`.
    pub saturation: f32,
    /// Value/brightness in `0..=1`.
    pub value: f32,
    /// Alpha in `0..=1`.
    pub alpha: f32,
}

impl ColourHsva {
    /// Converts an IR colour into HSV plus alpha.
    pub fn from_color(color: Color) -> Self {
        let r = color.r as f32 / 255.0;
        let g = color.g as f32 / 255.0;
        let b = color.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let hue = if delta <= f32::EPSILON {
            0.0
        } else if (max - r).abs() <= f32::EPSILON {
            60.0 * (((g - b) / delta) % 6.0)
        } else if (max - g).abs() <= f32::EPSILON {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        Self {
            hue: if hue < 0.0 { hue + 360.0 } else { hue },
            saturation: if max <= f32::EPSILON {
                0.0
            } else {
                delta / max
            },
            value: max,
            alpha: color.a as f32 / 255.0,
        }
    }

    /// Converts HSV plus alpha into an IR colour.
    pub fn to_color(self) -> Color {
        let h = self.hue.rem_euclid(360.0);
        let s = self.saturation.clamp(0.0, 1.0);
        let v = self.value.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
        let m = v - c;
        let (r1, g1, b1) = match h {
            h if h < 60.0 => (c, x, 0.0),
            h if h < 120.0 => (x, c, 0.0),
            h if h < 180.0 => (0.0, c, x),
            h if h < 240.0 => (0.0, x, c),
            h if h < 300.0 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Color {
            r: ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
            g: ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
            b: ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
            a: (self.alpha.clamp(0.0, 1.0) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8,
        }
    }
}

/// A customisable, controlled colour picker with Chrome, Sketch, Photoshop,
/// Compact, Circle, GitHub, Twitter, Material, Slider, Swatches, Block, Hue and
/// Alpha style layouts.
///
/// The widget is controlled: pass the current [`Color`] in [`ColourPicker::value`]
/// and update it in reducers. Swatches dispatch full colour values through
/// [`ColourPicker::on_change`]. Slider controls dispatch numeric channel updates
/// through the matching `on_*_change` action.
///
/// ```rust,ignore
/// ColourPicker {
///     value: view.state().accent,
///     variant: ColourPickerVariant::Chrome,
///     on_change: Some(Arc::new(move |colour| set_accent(colour))),
///     on_hue_change: Some(ctx.bind(SetAccentHue(0.0), reduce!(set_hue))),
///     ..Default::default()
/// }
/// .into()
/// ```
#[derive(Clone)]
pub struct ColourPicker {
    /// Explicit widget identity.
    pub id: Option<WidgetId>,
    /// Stable identifier prefix exposed to accessibility and LiveTest selectors.
    pub semantics_identifier: Option<String>,
    /// Currently selected colour.
    pub value: Color,
    /// Visual picker layout.
    pub variant: ColourPickerVariant,
    /// Optional palette. When empty, the variant's built-in palette is used.
    pub palette: Vec<Color>,
    /// Recent or user-saved colours shown after the main palette.
    pub recent: Vec<Color>,
    /// Whether alpha controls and alpha text are visible.
    pub show_alpha: bool,
    /// Whether hex/rgb/hsv text fields are visible for variants that support them.
    pub show_inputs: bool,
    /// Preferred picker width in layout points.
    pub width: Option<f32>,
    /// Swatch size in layout points.
    pub swatch_size: Option<f32>,
    /// Number of columns used by dense swatch layouts.
    pub columns: Option<usize>,
    /// Factory for full colour changes, usually used by swatch selection.
    pub on_change: Option<Arc<dyn Fn(Color) -> ActionEnvelope + Send + Sync>>,
    /// Action receiving `f32` hue degrees from hue sliders.
    pub on_hue_change: Option<ActionEnvelope>,
    /// Action receiving `f32` saturation in `0..=1`.
    pub on_saturation_change: Option<ActionEnvelope>,
    /// Action receiving `f32` value/brightness in `0..=1`.
    pub on_value_change: Option<ActionEnvelope>,
    /// Action receiving `f32` alpha in `0..=1`.
    pub on_alpha_change: Option<ActionEnvelope>,
    /// Action dispatched when the editable hex field changes. The new string
    /// is available through `ReducerContext::input.text_change()`.
    pub on_hex_input: Option<ActionEnvelope>,
}

/// American spelling alias for codebases that prefer `ColorPicker`.
pub type ColorPicker = ColourPicker;
/// American spelling alias for codebases that prefer `ColorPickerVariant`.
pub type ColorPickerVariant = ColourPickerVariant;

impl Default for ColourPicker {
    fn default() -> Self {
        Self {
            id: None,
            semantics_identifier: None,
            value: Color {
                r: 59,
                g: 130,
                b: 246,
                a: 255,
            },
            variant: ColourPickerVariant::Chrome,
            palette: Vec::new(),
            recent: Vec::new(),
            show_alpha: true,
            show_inputs: true,
            width: None,
            swatch_size: None,
            columns: None,
            on_change: None,
            on_hue_change: None,
            on_saturation_change: None,
            on_value_change: None,
            on_alpha_change: None,
            on_hex_input: None,
        }
    }
}

impl std::fmt::Debug for ColourPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColourPicker")
            .field("id", &self.id)
            .field("value", &self.value)
            .field("variant", &self.variant)
            .field("show_alpha", &self.show_alpha)
            .field("show_inputs", &self.show_inputs)
            .finish()
    }
}

impl ColourPicker {
    /// Sets the stable selector prefix for the picker and its generated controls.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<ColourPicker> for Widget {
    fn from(component: ColourPicker) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let viewport = view.viewport_size();
        let preferred_width = component.width.unwrap_or(match component.variant {
            ColourPickerVariant::Photoshop => 420.0,
            ColourPickerVariant::Compact | ColourPickerVariant::Github => 260.0,
            ColourPickerVariant::Hue | ColourPickerVariant::Alpha => 280.0,
            _ => 340.0,
        });
        let width = if viewport.width.is_finite() && viewport.width > 0.0 {
            preferred_width.min((viewport.width - 48.0).max(220.0))
        } else {
            preferred_width
        };
        let content_width = (width - 24.0).max(180.0);
        let palette = if component.palette.is_empty() {
            default_palette(component.variant)
        } else {
            component.palette.clone()
        };
        let hsva = ColourHsva::from_color(component.value);

        let body = match component.variant {
            ColourPickerVariant::Chrome | ColourPickerVariant::Sketch => editor_picker(
                &component,
                &palette,
                hsva,
                content_width,
                if component.variant == ColourPickerVariant::Sketch {
                    "Sketch"
                } else {
                    "Chrome"
                },
            ),
            ColourPickerVariant::Photoshop => {
                photoshop_picker(&component, &palette, hsva, content_width)
            }
            ColourPickerVariant::Compact => swatch_grid(&component, &palette, 20.0, 10, true),
            ColourPickerVariant::Circle => swatch_grid(&component, &palette, 30.0, 8, true),
            ColourPickerVariant::Github => {
                social_picker(&component, &palette, "GitHub labels", 26.0, 7)
            }
            ColourPickerVariant::Twitter => {
                social_picker(&component, &palette, "Twitter accents", 34.0, 7)
            }
            ColourPickerVariant::Material => material_picker(&component, &palette, content_width),
            ColourPickerVariant::Slider => slider_picker(&component, hsva, content_width, true),
            ColourPickerVariant::Swatches => swatches_picker(&component, &palette),
            ColourPickerVariant::Block => block_picker(&component, &palette, content_width),
            ColourPickerVariant::Hue => channel_slider(
                &component,
                "Hue",
                hsva.hue,
                0.0,
                360.0,
                component.on_hue_change.clone(),
                hue_fill(),
                "hue",
                content_width,
            ),
            ColourPickerVariant::Alpha => channel_slider(
                &component,
                "Alpha",
                hsva.alpha,
                0.0,
                1.0,
                component.on_alpha_change.clone(),
                alpha_fill(component.value),
                "alpha",
                content_width,
            ),
        };

        let mut root = Container::new(body)
            .width(width)
            .padding_all(12.0)
            .bg(tokens.colors.surface_raised)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large);
        root.id = component.id;
        root.into()
    }
}

fn editor_picker(
    picker: &ColourPicker,
    palette: &[Color],
    hsva: ColourHsva,
    width: f32,
    title: &str,
) -> Widget {
    Column {
        gap: Some(10.0),
        children: vec![
            header(picker, title),
            saturation_panel(picker, hsva, width),
            slider_picker(picker, hsva, width, false),
            inputs(picker, true),
            swatch_grid(
                picker,
                palette,
                picker.swatch_size.unwrap_or(22.0),
                10,
                false,
            ),
            recent_row(picker),
        ],
        ..Default::default()
    }
    .into()
}

fn photoshop_picker(
    picker: &ColourPicker,
    palette: &[Color],
    hsva: ColourHsva,
    width: f32,
) -> Widget {
    Row {
        gap: Some(12.0),
        align_items: AlignItems::Start,
        children: vec![
            Container::new(Column {
                gap: Some(10.0),
                children: vec![
                    saturation_panel(picker, hsva, 240.0),
                    channel_slider(
                        picker,
                        "Hue",
                        hsva.hue,
                        0.0,
                        360.0,
                        picker.on_hue_change.clone(),
                        hue_fill(),
                        "hue",
                        (width - 116.0).max(220.0),
                    ),
                    channel_slider(
                        picker,
                        "Saturation",
                        hsva.saturation,
                        0.0,
                        1.0,
                        picker.on_saturation_change.clone(),
                        saturation_fill(hsva),
                        "saturation",
                        (width - 116.0).max(220.0),
                    ),
                    channel_slider(
                        picker,
                        "Value",
                        hsva.value,
                        0.0,
                        1.0,
                        picker.on_value_change.clone(),
                        value_fill(hsva),
                        "value",
                        (width - 116.0).max(220.0),
                    ),
                    swatch_grid(picker, palette, 20.0, 8, false),
                ],
                ..Default::default()
            })
            .width((width - 116.0).max(220.0))
            .into(),
            Container::new(Column {
                gap: Some(8.0),
                children: vec![preview(picker, 76.0), inputs(picker, true)],
                ..Default::default()
            })
            .width(92.0)
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn slider_picker(
    picker: &ColourPicker,
    hsva: ColourHsva,
    width: f32,
    include_preview: bool,
) -> Widget {
    let mut children = Vec::new();
    if include_preview {
        children.push(
            Row {
                gap: Some(12.0),
                align_items: AlignItems::Center,
                children: vec![preview(picker, 44.0), inputs(picker, false)],
                ..Default::default()
            }
            .into(),
        );
    }
    children.extend([
        channel_slider(
            picker,
            "Hue",
            hsva.hue,
            0.0,
            360.0,
            picker.on_hue_change.clone(),
            hue_fill(),
            "hue",
            width,
        ),
        channel_slider(
            picker,
            "Saturation",
            hsva.saturation,
            0.0,
            1.0,
            picker.on_saturation_change.clone(),
            saturation_fill(hsva),
            "saturation",
            width,
        ),
        channel_slider(
            picker,
            "Value",
            hsva.value,
            0.0,
            1.0,
            picker.on_value_change.clone(),
            value_fill(hsva),
            "value",
            width,
        ),
    ]);
    if picker.show_alpha {
        children.push(channel_slider(
            picker,
            "Alpha",
            hsva.alpha,
            0.0,
            1.0,
            picker.on_alpha_change.clone(),
            alpha_fill(picker.value),
            "alpha",
            width,
        ));
    }

    Column {
        gap: Some(8.0),
        children,
        ..Default::default()
    }
    .into()
}

fn material_picker(picker: &ColourPicker, palette: &[Color], width: f32) -> Widget {
    Column {
        gap: Some(10.0),
        children: vec![
            header(picker, "Material"),
            Row {
                gap: Some(12.0),
                children: vec![preview(picker, 48.0), inputs(picker, false)],
                ..Default::default()
            }
            .into(),
            swatch_grid(
                picker,
                palette,
                34.0,
                ((width / 42.0) as usize).max(5),
                true,
            ),
        ],
        ..Default::default()
    }
    .into()
}

fn social_picker(
    picker: &ColourPicker,
    palette: &[Color],
    title: &str,
    size: f32,
    columns: usize,
) -> Widget {
    Column {
        gap: Some(10.0),
        children: vec![
            header(picker, title),
            Row {
                gap: Some(10.0),
                children: vec![
                    preview(picker, 38.0),
                    Text::new(hex_string(picker.value)).into(),
                ],
                ..Default::default()
            }
            .into(),
            swatch_grid(picker, palette, size, columns, true),
        ],
        ..Default::default()
    }
    .into()
}

fn swatches_picker(picker: &ColourPicker, palette: &[Color]) -> Widget {
    let groups = palette.chunks(7).map(|group| {
        Row {
            gap: Some(6.0),
            children: group
                .iter()
                .map(|color| swatch(picker, *color, 28.0, 8.0))
                .collect(),
            ..Default::default()
        }
        .into()
    });

    Column {
        gap: Some(8.0),
        children: std::iter::once(header(picker, "Swatches"))
            .chain(groups)
            .collect(),
        ..Default::default()
    }
    .into()
}

fn block_picker(picker: &ColourPicker, palette: &[Color], width: f32) -> Widget {
    Column {
        gap: Some(10.0),
        children: vec![
            Container::new(Text::new(hex_string(picker.value)).color(contrast_color(picker.value)))
                .height(92.0)
                .padding_all(14.0)
                .bg(picker.value)
                .border_radius(10.0)
                .into(),
            inputs(picker, false),
            swatch_grid(
                picker,
                palette,
                28.0,
                ((width / 36.0) as usize).max(6),
                true,
            ),
        ],
        ..Default::default()
    }
    .into()
}

fn header(picker: &ColourPicker, title: &str) -> Widget {
    Row {
        gap: Some(10.0),
        align_items: AlignItems::Center,
        children: vec![preview(picker, 28.0), Text::new(title).size(14.0).into()],
        ..Default::default()
    }
    .into()
}

fn saturation_panel(_picker: &ColourPicker, hsva: ColourHsva, width: f32) -> Widget {
    let base = ColourHsva {
        saturation: 1.0,
        value: 1.0,
        ..hsva
    }
    .to_color();
    Container::new(Text::new(""))
        .width(width)
        .height(138.0)
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 0.0),
            stops: vec![(0.0, Color::WHITE), (1.0, base)],
        })
        .border(
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 35,
            },
            1.0,
        )
        .border_radius(10.0)
        .into()
}

fn channel_slider(
    picker: &ColourPicker,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    action: Option<ActionEnvelope>,
    fill: Fill,
    semantic_suffix: &str,
    width: f32,
) -> Widget {
    let identifier = picker
        .semantics_identifier
        .as_ref()
        .map(|prefix| format!("{prefix}.{semantic_suffix}"));
    Container::new(Column {
        gap: Some(4.0),
        children: vec![
            Row {
                justify_content: JustifyContent::SpaceBetween,
                children: vec![
                    Text::new(label).size(12.0).into(),
                    Text::new(format_channel_value(label, value))
                        .size(12.0)
                        .into(),
                ],
                ..Default::default()
            }
            .into(),
            Container::new(Slider {
                semantics_identifier: identifier,
                value,
                min,
                max,
                track_height: Some(18.0),
                thumb_size: Some(16.0),
                track_fill: Some(Fill::Solid(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                })),
                on_change: action,
                ..Default::default()
            })
            .width(width)
            .height(18.0)
            .bg_fill(fill)
            .border_radius(8.0)
            .into(),
        ],
        ..Default::default()
    })
    .width(width)
    .into()
}

fn inputs(picker: &ColourPicker, expanded: bool) -> Widget {
    if !picker.show_inputs {
        return Container::new(Text::new(hex_string(picker.value))).into();
    }

    let hex = hex_string(picker.value);
    let mut children = vec![
        TextInput {
            semantics_identifier: picker
                .semantics_identifier
                .as_ref()
                .map(|prefix| format!("{prefix}.hex")),
            value: hex,
            width: Some(if expanded { 112.0 } else { 96.0 }),
            on_input: picker.on_hex_input.clone(),
            ..Default::default()
        }
        .into(),
        Text::new(format!(
            "rgb({}, {}, {})",
            picker.value.r, picker.value.g, picker.value.b
        ))
        .size(12.0)
        .into(),
    ];
    if expanded {
        let hsva = ColourHsva::from_color(picker.value);
        children.push(
            Text::new(format!(
                "hsv({:.0}, {:.0}%, {:.0}%)",
                hsva.hue,
                hsva.saturation * 100.0,
                hsva.value * 100.0
            ))
            .size(12.0)
            .into(),
        );
    }
    if picker.show_alpha {
        children.push(
            Text::new(format!("alpha {:.0}%", picker.value.a as f32 / 2.55))
                .size(12.0)
                .into(),
        );
    }

    Wrap {
        direction: FlexDirection::Row,
        spacing: Some(6.0),
        children,
    }
    .into()
}

fn preview(picker: &ColourPicker, size: f32) -> Widget {
    Container::new(Text::new(""))
        .size(size, size)
        .bg(picker.value)
        .border(
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 45,
            },
            1.0,
        )
        .border_radius((size / 5.0).clamp(6.0, 14.0))
        .into()
}

fn recent_row(picker: &ColourPicker) -> Widget {
    if picker.recent.is_empty() {
        return Container::new(Text::new("")).height(0.0).into();
    }

    Row {
        gap: Some(6.0),
        children: std::iter::once(Text::new("Recent").size(12.0).into())
            .chain(
                picker
                    .recent
                    .iter()
                    .map(|color| swatch(picker, *color, 18.0, 5.0)),
            )
            .collect(),
        ..Default::default()
    }
    .into()
}

fn swatch_grid(
    picker: &ColourPicker,
    palette: &[Color],
    default_size: f32,
    default_columns: usize,
    selected_label: bool,
) -> Widget {
    let size = picker.swatch_size.unwrap_or(default_size);
    let columns = picker.columns.unwrap_or(default_columns).max(1);
    let children = palette
        .iter()
        .map(|color| swatch(picker, *color, size, size / 2.8))
        .collect::<Vec<_>>();
    let grid = Wrap {
        direction: FlexDirection::Row,
        spacing: Some(6.0),
        children,
    };
    if selected_label {
        Column {
            gap: Some(8.0),
            children: vec![
                Text::new(format!("Selected {}", hex_string(picker.value)))
                    .size(12.0)
                    .into(),
                Container::new(grid)
                    .width((size + 6.0) * columns as f32)
                    .into(),
            ],
            ..Default::default()
        }
        .into()
    } else {
        Container::new(grid)
            .width((size + 6.0) * columns as f32)
            .into()
    }
}

fn swatch(picker: &ColourPicker, color: Color, size: f32, radius: f32) -> Widget {
    let selected = same_rgb_alpha(picker.value, color);
    let action = picker.on_change.as_ref().map(|callback| callback(color));
    let hex = hex_string(color);
    let id = picker
        .semantics_identifier
        .as_ref()
        .map(|prefix| format!("{prefix}.swatch.{}", hex.trim_start_matches('#')))
        .unwrap_or_else(|| format!("colour.swatch.{}", hex.trim_start_matches('#')));
    let semantics = Semantics {
        role: Role::Button,
        label: Some(format!("Select {hex}")),
        identifier: Some(id),
        checked: Some(selected),
        focusable: true,
        ..Default::default()
    };
    Button {
        child: None,
        on_press: action,
        width: Some(size),
        height: Some(size),
        padding: Some([0.0; 4]),
        variant: ButtonVariant::Ghost,
        background_fill: Some(Fill::Solid(color)),
        semantics: Some(semantics),
        ..Default::default()
    }
    .background_fill(Fill::Solid(color))
    .into_with_border(
        if selected {
            Color::BLACK
        } else {
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 45,
            }
        },
        radius,
    )
}

trait SwatchBorder {
    fn into_with_border(self, border: Color, radius: f32) -> Widget;
}

impl SwatchBorder for Button {
    fn into_with_border(self, border: Color, radius: f32) -> Widget {
        let width = self.width.unwrap_or(24.0);
        let height = self.height.unwrap_or(24.0);
        Container::new(self)
            .size(width, height)
            .border(border, 2.0)
            .border_radius(radius)
            .into()
    }
}

fn hue_fill() -> Fill {
    Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 0.0),
        stops: vec![
            (0.0, rgb(255, 0, 0)),
            (0.17, rgb(255, 255, 0)),
            (0.33, rgb(0, 255, 0)),
            (0.50, rgb(0, 255, 255)),
            (0.67, rgb(0, 0, 255)),
            (0.83, rgb(255, 0, 255)),
            (1.0, rgb(255, 0, 0)),
        ],
    }
}

fn saturation_fill(hsva: ColourHsva) -> Fill {
    Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 0.0),
        stops: vec![
            (
                0.0,
                ColourHsva {
                    saturation: 0.0,
                    ..hsva
                }
                .to_color(),
            ),
            (
                1.0,
                ColourHsva {
                    saturation: 1.0,
                    ..hsva
                }
                .to_color(),
            ),
        ],
    }
}

fn value_fill(hsva: ColourHsva) -> Fill {
    Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 0.0),
        stops: vec![
            (0.0, Color::BLACK),
            (1.0, ColourHsva { value: 1.0, ..hsva }.to_color()),
        ],
    }
}

fn alpha_fill(color: Color) -> Fill {
    Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 0.0),
        stops: vec![(0.0, color.with_alpha(0)), (1.0, color.with_alpha(255))],
    }
}

fn default_palette(variant: ColourPickerVariant) -> Vec<Color> {
    match variant {
        ColourPickerVariant::Material => material_palette(),
        ColourPickerVariant::Github => github_palette(),
        ColourPickerVariant::Twitter => twitter_palette(),
        ColourPickerVariant::Circle => circle_palette(),
        _ => standard_palette(),
    }
}

fn standard_palette() -> Vec<Color> {
    [
        "#D0021B", "#F5A623", "#F8E71C", "#8B572A", "#7ED321", "#417505", "#BD10E0", "#9013FE",
        "#4A90E2", "#50E3C2", "#B8E986", "#000000", "#4A4A4A", "#9B9B9B", "#FFFFFF",
    ]
    .iter()
    .filter_map(|value| parse_hex_color(value))
    .collect()
}

fn material_palette() -> Vec<Color> {
    [
        "#F44336", "#E91E63", "#9C27B0", "#673AB7", "#3F51B5", "#2196F3", "#03A9F4", "#00BCD4",
        "#009688", "#4CAF50", "#8BC34A", "#CDDC39", "#FFEB3B", "#FFC107", "#FF9800", "#FF5722",
        "#795548", "#607D8B",
    ]
    .iter()
    .filter_map(|value| parse_hex_color(value))
    .collect()
}

fn github_palette() -> Vec<Color> {
    [
        "#B60205", "#D93F0B", "#FBCA04", "#0E8A16", "#006B75", "#1D76DB", "#0052CC", "#5319E7",
        "#E99695", "#F9D0C4", "#FEF2C0", "#C2E0C6", "#BFDADC", "#BFD4F2", "#D4C5F9",
    ]
    .iter()
    .filter_map(|value| parse_hex_color(value))
    .collect()
}

fn twitter_palette() -> Vec<Color> {
    [
        "#1DA1F2", "#14171A", "#657786", "#AAB8C2", "#E1E8ED", "#F5F8FA", "#17BF63", "#F45D22",
        "#E0245E", "#794BC4", "#FFAD1F", "#2EC4B6",
    ]
    .iter()
    .filter_map(|value| parse_hex_color(value))
    .collect()
}

fn circle_palette() -> Vec<Color> {
    [
        "#F44336", "#FF9800", "#FFEB3B", "#4CAF50", "#00BCD4", "#2196F3", "#3F51B5", "#9C27B0",
        "#E91E63", "#795548", "#607D8B", "#000000",
    ]
    .iter()
    .filter_map(|value| parse_hex_color(value))
    .collect()
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let a = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).ok()?
    } else {
        255
    };
    Some(Color { r, g, b, a })
}

fn hex_string(color: Color) -> String {
    if color.a == 255 {
        format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r, color.g, color.b, color.a
        )
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

fn same_rgb_alpha(a: Color, b: Color) -> bool {
    a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a
}

fn contrast_color(color: Color) -> Color {
    let luma = (0.299 * color.r as f32 + 0.587 * color.g as f32 + 0.114 * color.b as f32) / 255.0;
    if luma > 0.58 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

fn format_channel_value(label: &str, value: f32) -> String {
    match label {
        "Hue" => format!("{value:.0} deg"),
        _ => format!("{:.0}%", value * 100.0),
    }
}
