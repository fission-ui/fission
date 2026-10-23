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

fission_component! {
/// The design tokens every widget draws from, plus the behaviour built on
/// them: grown hit targets and the loading state.
    pub(crate) FoundationsSection<GalleryState> {}
    |_section, _state, cx| { let tokens = cx.view.env().theme.tokens.clone(); } =>
        Column(
            gap: tokens.spacing.l,
            children: [
                // Groups sit further apart than the lines inside them, so each
                // reads as one unit (law of proximity).
                VStack(
                    spacing: tokens.spacing.l,
                    children: [
                        FoundationGroup("Look".into(), LookSwitcher()),
                        FoundationGroup("Density".into(), DensityDemo()),
                        FoundationGroup("Spacing".into(), SpacingScale()),
                        FoundationGroup("Hit targets".into(), HitTargetDemo()),
                        FoundationGroup("Loading".into(), LoadingDemo()),
                        FoundationGroup("State layers".into(), StateLayerSwatches()),
                        FoundationGroup("Colour pairs".into(), ColourPairSwatches()),
                        FoundationGroup("Icon sizes".into(), IconSizeScale()),
                        FoundationGroup("Window class".into(), WindowClassReadout()),
                    ],
                ),
            ],
        )
}

fission_component! {
/// A labelled block inside the Foundations section.
    FoundationGroup<()> { title: String, child: Widget }
    |group, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(group.title.clone())
                    .size(tokens.typography.font_size_base)
                    .weight(tokens.typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
                group.child.clone(),
            ],
        )
}

fission_component! {
/// Look, mode and density, pinned above the page so any section can be
/// checked in any theme without scrolling back to Foundations.
    pub(crate) ThemeBar<GalleryState> {}
    |_bar, state, cx| {
        let tokens = cx.view.env().theme.tokens.clone();
        let set_look = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetLook(0), set_look);
            move |index| action.with_action(&SetLook(index))
        });
        let set_mode = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetMode(0), set_mode);
            move |index| action.with_action(&SetMode(index))
        });
        let set_density = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetDensity(0), set_density);
            move |index| action.with_action(&SetDensity(index))
        });
        let density_index = DENSITIES
            .iter()
            .position(|density| *density == state.density)
            .unwrap_or(0);
    } =>
        SemanticsRegion(
            // A plain row: it measures its own height, so the bar holds its controls
            // instead of collapsing and letting the page paint over them.
            // Controls wrap onto further lines when the window is too narrow for one.
            Container(Row(
                gap: tokens.spacing.m,
                line_gap: tokens.spacing.s,
                wrap: fission::op::FlexWrap::Wrap,
                align_items: fission::op::AlignItems::Center,
                children: [
                    Text("Theme")
                        .weight(tokens.typography.font_weight_semibold)
                        .color(tokens.colors.text_primary),
                    if state.native_look {
                        Text("Native platform look").color(tokens.colors.text_secondary)
                    } else {
                        SegmentedControl(
                            options: vec!["Tidewater".into(), "Graphite".into(), "Ember".into()],
                            selected_index: state.look,
                            on_change: set_look,
                        )
                    },
                    SegmentedControl(
                        options: vec!["Light".into(), "Dark".into()],
                        selected_index: usize::from(state.dark_mode),
                        on_change: set_mode,
                    ),
                    SegmentedControl(
                        options: vec!["Compact".into(), "Comfortable".into(), "Spacious".into()],
                        selected_index: density_index,
                        on_change: set_density,
                    ),
                ],
            ))
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
}

fission_component! {
    LookSwitcher<GalleryState> {}
    |_switcher, state, cx| {
        let tokens = cx.view.env().theme.tokens.clone();
        let set_look = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetLook(0), set_look);
            move |index| action.with_action(&SetLook(index))
        });
        let set_mode = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetMode(0), set_mode);
            move |index| action.with_action(&SetMode(index))
        });
        let toggle_native = with_reducer!(cx.build, ToggleNativeLook, toggle_native_look);
        let set_native_preview = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetNativePreview(0), set_native_preview);
            move |index| action.with_action(&SetNativePreview(index))
        });
    } =>
        VStack(
            spacing: tokens.spacing.ms,
            children: [
                Text(
                    "Tidewater is Fission's default design system. Graphite and Ember ship \
                     as presets."
                )
                .color(tokens.colors.text_secondary),
                Wrap(
                    direction: FlexDirection::Row,
                    spacing: tokens.spacing.m,
                    run_spacing: tokens.spacing.s,
                    children: [
                        SegmentedControl(
                            options: vec!["Tidewater".into(), "Graphite".into(), "Ember".into()],
                            selected_index: state.look,
                            on_change: set_look,
                        ),
                        SegmentedControl(
                            options: vec!["Light".into(), "Dark".into()],
                            selected_index: usize::from(state.dark_mode),
                            on_change: set_mode,
                        ),
                    ],
                ),
                HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        Switch(
                            checked: state.native_look,
                            on_toggle: toggle_native,
                        )
                        .semantics_identifier("gallery.foundations.native_look"),
                        Text("Native platform look").color(tokens.colors.text_primary),
                    ],
                ),
                if state.native_look {
                    SegmentedControl(
                        options: vec![
                            "This device".into(),
                            "Apple".into(),
                            "Android".into(),
                            "Windows".into(),
                        ],
                        selected_index: state.native_preview,
                        on_change: set_native_preview,
                    )
                } else {
                    Spacer()
                },
            ],
        )
}

fission_component! {
    DensityDemo<GalleryState> {}
    |_demo, state, cx| {
        let tokens = cx.view.env().theme.tokens.clone();
        let set_density = std::sync::Arc::new({
            let action = with_reducer!(cx.build, SetDensity(0), set_density);
            move |index| action.with_action(&SetDensity(index))
        });
        let selected_index = DENSITIES
            .iter()
            .position(|density| *density == state.density)
            .unwrap_or(0);
        let sizing = &tokens.sizing;

    } =>
        VStack(
            spacing: tokens.spacing.ms,
            children: [
                Text(format!(
                    "Controls step {step:.0}px between densities, from the design system's \
                     comfortable sizes. Heights now: {sm:.0} / {md:.0} / {lg:.0} / {xl:.0}px.",
                    step = sizing.density_step,
                    sm = sizing.control_sm,
                    md = sizing.control_md,
                    lg = sizing.control_lg,
                    xl = sizing.control_xl,
                ))
                .color(tokens.colors.text_secondary),
                Container(SegmentedControl(
                    options: vec!["Compact".into(), "Comfortable".into(), "Spacious".into()],
                    selected_index: selected_index,
                    on_change: set_density,
                ))
                .max_width_length(Length::points(420.0)),
                Wrap(
                    direction: FlexDirection::Row,
                    spacing: tokens.spacing.s,
                    run_spacing: tokens.spacing.s,
                    children: [
                        Button(
                            size: ComponentSize::Sm,
                            child: Text("Small"),
                        ),
                        Button(child: Text("Medium")),
                        Button(
                            size: ComponentSize::Lg,
                            variant: ButtonVariant::Outline,
                            child: Text("Large"),
                        ),
                        Button(
                            size: ComponentSize::Xl,
                            child: Text("Extra large"),
                        ),
                    ],
                ),
                Container(TextInput(
                    id: WidgetId::explicit("gallery.foundations.density_input"),
                    placeholder: Some("Search messages".into()),
                ))
                .max_width_length(Length::points(320.0)),
            ],
        )
}

fission_component! {
    SpacingScale<()> {}
    |_scale, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
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
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.m,
            run_spacing: tokens.spacing.s,
            children: [
                for (name, value) in steps {
                    VStack(
                        key: name,
                        spacing: tokens.spacing.xs,
                        children: [
                            Container(Row())
                                .size(value.max(1.0), tokens.spacing.ms)
                                .bg(tokens.colors.primary)
                                .border_radius(tokens.radii.none),
                            Text(format!("{name} {value:.0}"))
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary),
                        ],
                    )
                },
            ],
        )
}

fission_component! {
    HitTargetDemo<GalleryState> {}
    |_demo, state, cx| {
        let tokens = cx.view.env().theme.tokens.clone();
        let press = with_reducer!(cx.build, PressHitTarget, press_hit_target);
        let toggle = with_reducer!(cx.build, ToggleHitAreas, toggle_hit_areas);

        let dot = tokens.sizing.icon_sm;
        let target = tokens.sizing.min_pointer_target;
        let inset = ((target - dot) / 2.0).max(0.0);

    } =>
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(format!(
                    "Each dot is {dot:.0}px but answers to a {target:.0}px pointer area and a \
                     {touch:.0}px touch area. Click just outside one.",
                    touch = tokens.sizing.min_touch_target,
                ))
                .color(tokens.colors.text_secondary),
                HStack(
                    spacing: tokens.spacing.ms,
                    children: [
                        HStack(
                            spacing: 0.0_f32,
                            children: [
                                for index in 0..HIT_TARGETS {
                                    HitTarget(index, dot, inset, press.clone(), key: index)
                                },
                            ],
                        ),
                        Text(format!("Pressed {} times", state.hit_target_presses))
                            .color(tokens.colors.text_primary),
                    ],
                ),
                Checkbox(
                    checked: state.show_hit_areas,
                    on_toggle: toggle,
                    label: Some("Show pointer hit areas".to_string()),
                )
                .semantics_identifier("gallery.foundations.show_hit_areas"),
            ],
        )
}

fission_component! {
    HitTarget<GalleryState> { index: usize, dot: f32, inset: f32, action: ActionEnvelope }
    |target, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        if state.show_hit_areas {
            Container(
                Button(
                    id: WidgetId::explicit(&format!("gallery.hit_target.{}", target.index)),
                    width: target.dot,
                    height: target.dot,
                    padding: [0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32],
                    on_press: target.action.clone(),
                    motion: ButtonMotion::None,
                    semantics: Semantics {
                        role: Role::Button,
                        label: Some(format!("Target {}", target.index + 1)),
                        identifier: Some(format!("gallery.foundations.hit_target.{}", target.index)),
                        focusable: true,
                        ..Semantics::default()
                    },
                )
            )
            .padding_all(target.inset)
            .bg(tokens.colors.primary.with_alpha(40))
            .border_radius(tokens.radii.small)
        } else {
            Container(
                Button(
                    id: WidgetId::explicit(&format!("gallery.hit_target.{}", target.index)),
                    width: target.dot,
                    height: target.dot,
                    padding: [0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32],
                    on_press: target.action.clone(),
                    motion: ButtonMotion::None,
                    semantics: Semantics {
                        role: Role::Button,
                        label: Some(format!("Target {}", target.index + 1)),
                        identifier: Some(format!("gallery.foundations.hit_target.{}", target.index)),
                        focusable: true,
                        ..Semantics::default()
                    },
                )
            )
            .padding_all(target.inset)
        }
}

fission_component! {
    LoadingDemo<GalleryState> {}
    |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let press = with_reducer!(cx.build, PressLoadingButton, press_loading_button);
        let toggle = with_reducer!(cx.build, ToggleSlowWork, toggle_slow_work);
        let loading = state.foundation_loading;

    } =>
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(
                    "A loading button keeps its size and colour, swaps its label for a spinning ring, \
                     ignores presses and reports itself busy."
                )
                .color(tokens.colors.text_secondary),
                Wrap(
                    direction: FlexDirection::Row,
                    spacing: tokens.spacing.s,
                    run_spacing: None,
                    children: [
                        Button(
                            id: WidgetId::explicit("gallery.foundations.save"),
                            child: Text("Save changes"),
                            on_press: press.clone(),
                            loading: loading,
                        )
                        .semantics_identifier("gallery.foundations.save"),
                        Button(
                            id: WidgetId::explicit("gallery.foundations.export"),
                            variant: ButtonVariant::Outline,
                            child: Text("Export report"),
                            on_press: press,
                            loading: loading,
                        )
                        .semantics_identifier("gallery.foundations.export"),
                    ],
                ),
                HStack(
                    spacing: tokens.spacing.m,
                    children: [
                        Switch(
                            checked: loading,
                            on_toggle: toggle,
                        )
                        .semantics_identifier("gallery.foundations.slow_work"),
                        Text("Simulate slow work").color(tokens.colors.text_primary),
                        Text(format!("Presses {}", state.loading_presses))
                            .color(tokens.colors.text_secondary),
                    ],
                ),
            ],
        )
}

fission_component! {
    StateLayerSwatches<()> {}
    |_swatches, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
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
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.m,
            run_spacing: tokens.spacing.s,
            children: [
                for (name, value) in layers {
                    VStack(
                        key: name,
                        spacing: tokens.spacing.xs,
                        children: [
                            Container(Row())
                                .size(swatch * 1.5, swatch)
                                .bg(tokens
                                    .colors
                                    .primary
                                    .with_alpha((value.clamp(0.0, 1.0) * 255.0).round() as u8))
                                .border(tokens.colors.border, tokens.sizing.border_hairline)
                                .border_radius(tokens.radii.medium),
                            Text(format!("{name} {:.0}%", value * 100.0))
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary),
                        ],
                    )
                },
            ],
        )
}

fission_component! {
/// Each paired role's fill with content drawn in its own foreground colour.
    ColourPairSwatches<()> {}
    |_swatches, _state, cx| {
        use fission::theme::ColorRole;
        let tokens = &cx.view.env().theme.tokens;
        let roles = [
            ("background", ColorRole::Background),
            ("surface", ColorRole::Surface),
            ("card", ColorRole::Card),
            ("popover", ColorRole::Popover),
            ("muted", ColorRole::Muted),
            ("primary", ColorRole::Primary),
            ("secondary", ColorRole::Secondary),
            ("accent", ColorRole::Accent),
            ("destructive", ColorRole::Destructive),
        ];
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.s,
            run_spacing: tokens.spacing.s,
            children: [
                for (name, role) in roles {
                    ColourPairSwatch(name, role, key: name)
                },
            ],
        )
}

fission_component! {
    ColourPairSwatch<()> { name: &'static str, role: fission::theme::ColorRole }
    |swatch, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let pair = tokens.colors.pair(swatch.role);
    } =>
        Container(
            VStack(
                spacing: tokens.spacing.xxs,
                children: [
                    Text("Aa")
                        .size(tokens.typography.font_size_lg)
                        .weight(tokens.typography.font_weight_semibold)
                        .color(pair.on),
                    Text(swatch.name)
                        .size(tokens.typography.font_size_xs)
                        .color(pair.on),
                ],
            )
        )
        .width(tokens.sizing.control_xl * 2.0)
        .padding_all(tokens.spacing.s)
        .bg(pair.fill)
        .border(tokens.colors.border, tokens.sizing.border_hairline)
        .border_radius(tokens.radii.medium)
}

fission_component! {
    IconSizeScale<()> {}
    |_scale, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let sizing = &tokens.sizing;
        let sizes = [
            ("xs", sizing.icon_xs),
            ("sm", sizing.icon_sm),
            ("md", sizing.icon_md),
            ("lg", sizing.icon_lg),
            ("xl", sizing.icon_xl),
        ];
    } =>
        HStack(
            spacing: tokens.spacing.l,
            children: [
                for (name, size) in sizes {
                    VStack(
                        key: name,
                        spacing: tokens.spacing.xs,
                        children: [
                            Container(Row())
                                .size(size, size)
                                .bg(tokens.colors.text_secondary)
                                .border_radius(tokens.radii.small),
                            Text(format!("{name} {size:.0}"))
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary),
                        ],
                    )
                },
            ],
        )
}

fission_component! {
    WindowClassReadout<()> {}
    |_readout, _state, cx| {
        let env = cx.view.env();
        let tokens = &env.theme.tokens;
        let width = env.viewport_size.width;
        let class = match tokens.breakpoints.class_for(width) {
            WindowClass::Compact => "Compact",
            WindowClass::Medium => "Medium",
            WindowClass::Expanded => "Expanded",
            WindowClass::Large => "Large",
            WindowClass::ExtraLarge => "Extra large",
        };
    } =>
        Text::new(format!(
            "{class}, {width:.0}px wide. Resize the window to cross a breakpoint \
             ({:.0} / {:.0} / {:.0} / {:.0}px).",
            tokens.breakpoints.compact_max,
            tokens.breakpoints.medium_max,
            tokens.breakpoints.expanded_max,
            tokens.breakpoints.large_max,
        ))
        .color(tokens.colors.text_primary)
}
