use crate::gallery_section::GallerySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::theme::{Density, WindowClass};
use fission::widgets::{HStack, SegmentedControl, VStack, Wrap};

const HIT_TARGETS: usize = 4;

const DENSITIES: [Density; 3] = [Density::Compact, Density::Comfortable, Density::Spacious];

#[fission_reducer(SetDensity)]
fn set_density(state: &mut GalleryState, index: usize) {
    if let Some(density) = DENSITIES.get(index) {
        state.density = *density;
    }
}

#[fission_reducer(SetLook)]
fn set_look(state: &mut GalleryState, index: usize) {
    state.look = index.min(2);
}

#[fission_reducer(ToggleNativeLook)]
fn toggle_native_look(state: &mut GalleryState) {
    state.native_look = !state.native_look;
}

#[fission_reducer(SetNativePreview)]
fn set_native_preview(state: &mut GalleryState, index: usize) {
    state.native_preview = index.min(3);
}

#[fission_reducer(SetMode)]
fn set_mode(state: &mut GalleryState, index: usize) {
    state.dark_mode = index == 1;
}

#[fission_reducer(ToggleHitAreas)]
fn toggle_hit_areas(state: &mut GalleryState) {
    state.show_hit_areas = !state.show_hit_areas;
}

#[fission_reducer(PressHitTarget)]
fn press_hit_target(state: &mut GalleryState) {
    state.hit_target_presses += 1;
}

#[fission_reducer(ToggleSlowWork)]
fn toggle_slow_work(state: &mut GalleryState) {
    state.foundation_loading = !state.foundation_loading;
}

#[fission_reducer(PressLoadingButton)]
fn press_loading_button(state: &mut GalleryState) {
    state.loading_presses += 1;
}

/// The design tokens every widget draws from, plus the behaviour built on
/// them: grown hit targets and the loading state.
pub(crate) struct FoundationsSection;

impl From<FoundationsSection> for Widget {
    fn from(_section: FoundationsSection) -> Self {
        let tokens = {
            let (_, view) = fission::build::current::<GalleryState>();
            view.env().theme.tokens.clone()
        };

        GallerySection::new(
            "Foundations",
            widgets![
                Text::new(
                    "Tokens every widget draws from. A design system's DSP file sets them; \
                     anything it leaves out falls back to Fission's defaults."
                )
                .color(tokens.colors.text_secondary),
                // Groups sit further apart than the lines inside them, so each
                // reads as one unit (law of proximity).
                VStack {
                    spacing: Some(tokens.spacing.l),
                    children: widgets![
                        FoundationGroup {
                            title: "Look".into(),
                            child: LookSwitcher.into(),
                        },
                        FoundationGroup {
                            title: "Density".into(),
                            child: DensityDemo.into(),
                        },
                        FoundationGroup {
                            title: "Spacing".into(),
                            child: SpacingScale.into(),
                        },
                        FoundationGroup {
                            title: "Hit targets".into(),
                            child: HitTargetDemo.into(),
                        },
                        FoundationGroup {
                            title: "Loading".into(),
                            child: LoadingDemo.into(),
                        },
                        FoundationGroup {
                            title: "State layers".into(),
                            child: StateLayerSwatches.into(),
                        },
                        FoundationGroup {
                            title: "Icon sizes".into(),
                            child: IconSizeScale.into(),
                        },
                        FoundationGroup {
                            title: "Window class".into(),
                            child: WindowClassReadout.into(),
                        },
                    ],
                },
            ],
        )
        .into()
    }
}

/// A labelled block inside the Foundations section.
struct FoundationGroup {
    title: String,
    child: Widget,
}

impl From<FoundationGroup> for Widget {
    fn from(group: FoundationGroup) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        VStack {
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Text::new(group.title)
                    .size(tokens.typography.font_size_base)
                    .weight(tokens.typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
                group.child,
            ],
        }
        .into()
    }
}

/// Look, mode and density, pinned above the page so any section can be
/// checked in any theme without scrolling back to Foundations.
pub(crate) struct ThemeBar;

impl From<ThemeBar> for Widget {
    fn from(_bar: ThemeBar) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = view.env().theme.tokens.clone();
        let set_look = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetLook(0), set_look);
            move |index| action.with_action(&SetLook(index))
        });
        let set_mode = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetMode(0), set_mode);
            move |index| action.with_action(&SetMode(index))
        });
        let set_density = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetDensity(0), set_density);
            move |index| action.with_action(&SetDensity(index))
        });
        let density_index = DENSITIES
            .iter()
            .position(|density| *density == state.density)
            .unwrap_or(0);
        // The native look replaces the theme, so the look picker says so rather
        // than pretending a preset is active.
        let look_control: Widget = if state.native_look {
            Text::new("Native platform look")
                .color(tokens.colors.text_secondary)
                .into()
        } else {
            SegmentedControl {
                options: vec!["Tidewater".into(), "Graphite".into(), "Ember".into()],
                selected_index: state.look,
                on_change: Some(set_look),
            }
            .into()
        };

        SemanticsRegion::new(
            // A plain row: it measures its own height, so the bar holds its controls
            // instead of collapsing and letting the page paint over them.
            Container::new(Row {
                gap: Some(tokens.spacing.m),
                align_items: fission::op::AlignItems::Center,
                children: widgets![
                    Text::new("Theme")
                        .weight(tokens.typography.font_weight_semibold)
                        .color(tokens.colors.text_primary),
                    look_control,
                    SegmentedControl {
                        options: vec!["Light".into(), "Dark".into()],
                        selected_index: usize::from(state.dark_mode),
                        on_change: Some(set_mode),
                    },
                    SegmentedControl {
                        options: vec!["Compact".into(), "Comfortable".into(), "Spacious".into()],
                        selected_index: density_index,
                        on_change: Some(set_density),
                    },
                ],
                ..Default::default()
            })
            .width_length(Length::percent(100.0))
            .padding_lengths(Length::symmetric(
                Length::points(tokens.spacing.l),
                Length::points(tokens.spacing.s),
            ))
            .bg(tokens.colors.surface)
            .border_bottom(tokens.colors.border, tokens.sizing.border_hairline),
        )
        .role(Role::Toolbar)
        .label("Theme")
        .identifier("gallery.theme_bar")
        .into()
    }
}

struct LookSwitcher;

impl From<LookSwitcher> for Widget {
    fn from(_switcher: LookSwitcher) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = view.env().theme.tokens.clone();
        let set_look = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetLook(0), set_look);
            move |index| action.with_action(&SetLook(index))
        });
        let set_mode = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetMode(0), set_mode);
            move |index| action.with_action(&SetMode(index))
        });
        let toggle_native = with_reducer!(ctx, ToggleNativeLook, toggle_native_look);
        let set_native_preview = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetNativePreview(0), set_native_preview);
            move |index| action.with_action(&SetNativePreview(index))
        });
        // Progressive disclosure: the platform preview appears only once the
        // native look is on.
        let native_preview: Widget = if state.native_look {
            SegmentedControl {
                options: vec![
                    "This device".into(),
                    "Apple".into(),
                    "Android".into(),
                    "Windows".into(),
                ],
                selected_index: state.native_preview,
                on_change: Some(set_native_preview),
            }
            .into()
        } else {
            Spacer::default().into()
        };

        VStack {
            spacing: Some(tokens.spacing.ms),
            children: widgets![
                Text::new(
                    "Tidewater is Fission's default design system. Graphite and Ember ship \
                     as presets."
                )
                .color(tokens.colors.text_secondary),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.m),
                    run_spacing: Some(tokens.spacing.s),
                    children: widgets![
                        SegmentedControl {
                            options: vec!["Tidewater".into(), "Graphite".into(), "Ember".into()],
                            selected_index: state.look,
                            on_change: Some(set_look),
                        },
                        SegmentedControl {
                            options: vec!["Light".into(), "Dark".into()],
                            selected_index: usize::from(state.dark_mode),
                            on_change: Some(set_mode),
                        },
                    ],
                },
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Switch {
                            checked: state.native_look,
                            on_toggle: Some(toggle_native),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.foundations.native_look"),
                        Text::new("Native platform look").color(tokens.colors.text_primary),
                    ],
                },
                native_preview,
            ],
        }
        .into()
    }
}

struct DensityDemo;

impl From<DensityDemo> for Widget {
    fn from(_demo: DensityDemo) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = view.env().theme.tokens.clone();
        let set_density = std::sync::Arc::new({
            let action = with_reducer!(ctx, SetDensity(0), set_density);
            move |index| action.with_action(&SetDensity(index))
        });
        let selected_index = DENSITIES
            .iter()
            .position(|density| *density == state.density)
            .unwrap_or(0);
        let sizing = &tokens.sizing;

        VStack {
            spacing: Some(tokens.spacing.ms),
            children: widgets![
                Text::new(format!(
                    "Controls step {step:.0}px between densities, from the design system's \
                     comfortable sizes. Heights now: {sm:.0} / {md:.0} / {lg:.0} / {xl:.0}px.",
                    step = sizing.density_step,
                    sm = sizing.control_sm,
                    md = sizing.control_md,
                    lg = sizing.control_lg,
                    xl = sizing.control_xl,
                ))
                .color(tokens.colors.text_secondary),
                Container::new(SegmentedControl {
                    options: vec!["Compact".into(), "Comfortable".into(), "Spacious".into()],
                    selected_index,
                    on_change: Some(set_density),
                })
                .max_width_length(Length::points(420.0)),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    run_spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Button {
                            size: ComponentSize::Sm,
                            child: Some(Text::new("Small").into()),
                            ..Default::default()
                        },
                        Button {
                            child: Some(Text::new("Medium").into()),
                            ..Default::default()
                        },
                        Button {
                            size: ComponentSize::Lg,
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Large").into()),
                            ..Default::default()
                        },
                        Button {
                            size: ComponentSize::Xl,
                            child: Some(Text::new("Extra large").into()),
                            ..Default::default()
                        },
                    ],
                },
                Container::new(TextInput {
                    id: Some(WidgetId::explicit("gallery.foundations.density_input")),
                    placeholder: Some("Search messages".into()),
                    ..Default::default()
                })
                .max_width_length(Length::points(320.0)),
            ],
        }
        .into()
    }
}

struct SpacingScale;

impl From<SpacingScale> for Widget {
    fn from(_scale: SpacingScale) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let spacing = &tokens.spacing;
        let steps = [
            ("xxs", spacing.xxs),
            ("xs", spacing.xs),
            ("s", spacing.s),
            ("ms", spacing.ms),
            ("m", spacing.m),
            ("ml", spacing.ml),
            ("l", spacing.l),
            ("xl", spacing.xl),
            ("xxl", spacing.xxl),
        ];
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.m),
            run_spacing: Some(tokens.spacing.s),
            children: steps
                .into_iter()
                .map(|(name, value)| {
                    VStack {
                        spacing: Some(tokens.spacing.xs),
                        children: widgets![
                            Container::new(Row::default())
                                .size(value.max(1.0), tokens.spacing.ms)
                                .bg(tokens.colors.primary)
                                .border_radius(tokens.radii.none),
                            Text::new(format!("{name} {value:.0}"))
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary),
                        ],
                    }
                    .into()
                })
                .collect(),
        }
        .into()
    }
}

struct HitTargetDemo;

impl From<HitTargetDemo> for Widget {
    fn from(_demo: HitTargetDemo) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = view.env().theme.tokens.clone();
        let press = with_reducer!(ctx, PressHitTarget, press_hit_target);
        let toggle = with_reducer!(ctx, ToggleHitAreas, toggle_hit_areas);

        let dot = tokens.sizing.icon_sm;
        let target = tokens.sizing.min_pointer_target;
        let inset = ((target - dot) / 2.0).max(0.0);

        let targets = (0..HIT_TARGETS)
            .map(|index| {
                let button = Button {
                    id: Some(WidgetId::explicit(&format!("gallery.hit_target.{index}"))),
                    width: Some(dot),
                    height: Some(dot),
                    padding: Some([0.0; 4]),
                    on_press: Some(press.clone()),
                    motion: Some(ButtonMotion::None),
                    semantics: Some(Semantics {
                        role: Role::Button,
                        label: Some(format!("Target {}", index + 1)),
                        identifier: Some(format!("gallery.foundations.hit_target.{index}")),
                        focusable: true,
                        ..Semantics::default()
                    }),
                    ..Default::default()
                };
                let area = Container::new(button).padding_all(inset);
                if state.show_hit_areas {
                    area.bg(tokens.colors.primary.with_alpha(40))
                        .border_radius(tokens.radii.small)
                        .into()
                } else {
                    area.into()
                }
            })
            .collect::<Vec<Widget>>();

        VStack {
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Text::new(format!(
                    "Each dot is {dot:.0}px but answers to a {target:.0}px pointer area and a \
                     {touch:.0}px touch area. Click just outside one.",
                    touch = tokens.sizing.min_touch_target,
                ))
                .color(tokens.colors.text_secondary),
                HStack {
                    spacing: Some(tokens.spacing.ms),
                    children: widgets![
                        HStack {
                            spacing: Some(0.0),
                            children: targets,
                        },
                        Text::new(format!("Pressed {} times", state.hit_target_presses))
                            .color(tokens.colors.text_primary),
                    ],
                },
                Checkbox {
                    checked: state.show_hit_areas,
                    on_toggle: Some(toggle),
                    label: Some("Show pointer hit areas".into()),
                    ..Default::default()
                }
                .semantics_identifier("gallery.foundations.show_hit_areas"),
            ],
        }
        .into()
    }
}

struct LoadingDemo;

impl From<LoadingDemo> for Widget {
    fn from(_demo: LoadingDemo) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let press = with_reducer!(ctx, PressLoadingButton, press_loading_button);
        let toggle = with_reducer!(ctx, ToggleSlowWork, toggle_slow_work);
        let loading = state.foundation_loading;

        VStack {
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Text::new(
                    "A loading button keeps its size and colour, swaps its label for a spinning ring, \
                     ignores presses and reports itself busy."
                )
                .color(tokens.colors.text_secondary),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    run_spacing: None,
                    children: widgets![
                        Button {
                            id: Some(WidgetId::explicit("gallery.foundations.save")),
                            child: Some(Text::new("Save changes").into()),
                            on_press: Some(press.clone()),
                            loading,
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.foundations.save"),
                        Button {
                            id: Some(WidgetId::explicit("gallery.foundations.export")),
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Export report").into()),
                            on_press: Some(press),
                            loading,
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.foundations.export"),
                    ],
                },
                HStack {
                    spacing: Some(tokens.spacing.m),
                    children: widgets![
                        Switch {
                            checked: loading,
                            on_toggle: Some(toggle),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.foundations.slow_work"),
                        Text::new("Simulate slow work").color(tokens.colors.text_primary),
                        Text::new(format!("Presses {}", state.loading_presses))
                            .color(tokens.colors.text_secondary),
                    ],
                },
            ],
        }
        .into()
    }
}

struct StateLayerSwatches;

impl From<StateLayerSwatches> for Widget {
    fn from(_swatches: StateLayerSwatches) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let opacity = &tokens.opacity;
        let layers = [
            ("Hover", opacity.hover_layer),
            ("Focus", opacity.focus_layer),
            ("Pressed", opacity.pressed_layer),
            ("Selected", opacity.selected_layer),
            ("Dragged", opacity.dragged_layer),
            ("Disabled", opacity.disabled),
        ];
        let swatch = tokens.sizing.control_xl;
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.m),
            run_spacing: Some(tokens.spacing.s),
            children: layers
                .into_iter()
                .map(|(name, value)| {
                    VStack {
                        spacing: Some(tokens.spacing.xs),
                        children: widgets![
                            Container::new(Row::default())
                                .size(swatch * 1.5, swatch)
                                .bg(tokens
                                    .colors
                                    .primary
                                    .with_alpha((value.clamp(0.0, 1.0) * 255.0).round() as u8))
                                .border(tokens.colors.border, tokens.sizing.border_hairline)
                                .border_radius(tokens.radii.medium),
                            Text::new(format!("{name} {:.0}%", value * 100.0))
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary),
                        ],
                    }
                    .into()
                })
                .collect(),
        }
        .into()
    }
}

struct IconSizeScale;

impl From<IconSizeScale> for Widget {
    fn from(_scale: IconSizeScale) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let sizing = &tokens.sizing;
        let sizes = [
            ("xs", sizing.icon_xs),
            ("sm", sizing.icon_sm),
            ("md", sizing.icon_md),
            ("lg", sizing.icon_lg),
            ("xl", sizing.icon_xl),
        ];
        HStack {
            spacing: Some(tokens.spacing.l),
            children: sizes
                .into_iter()
                .map(|(name, size)| {
                    VStack {
                        spacing: Some(tokens.spacing.xs),
                        children: widgets![
                            Container::new(Row::default())
                                .size(size, size)
                                .bg(tokens.colors.text_secondary)
                                .border_radius(tokens.radii.small),
                            Text::new(format!("{name} {size:.0}"))
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary),
                        ],
                    }
                    .into()
                })
                .collect(),
        }
        .into()
    }
}

struct WindowClassReadout;

impl From<WindowClassReadout> for Widget {
    fn from(_readout: WindowClassReadout) -> Self {
        let (_, view) = fission::build::current::<()>();
        let env = view.env();
        let tokens = &env.theme.tokens;
        let width = env.viewport_size.width;
        let class = match tokens.breakpoints.class_for(width) {
            WindowClass::Compact => "Compact",
            WindowClass::Medium => "Medium",
            WindowClass::Expanded => "Expanded",
            WindowClass::Large => "Large",
            WindowClass::ExtraLarge => "Extra large",
        };
        Text::new(format!(
            "{class}, {width:.0}px wide. Resize the window to cross a breakpoint \
             ({:.0} / {:.0} / {:.0} / {:.0}px).",
            tokens.breakpoints.compact_max,
            tokens.breakpoints.medium_max,
            tokens.breakpoints.expanded_max,
            tokens.breakpoints.large_max,
        ))
        .color(tokens.colors.text_primary)
        .into()
    }
}
