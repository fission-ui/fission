use fission_core::authoring::LoweringCx;
use fission_core::ui::widgets::button::{ButtonContent, ButtonIconContent};
use fission_core::ui::{Button, ButtonStyleOverride, ButtonVariant, Icon, Text, TextInput};
use fission_core::{Env, LayoutSize, RuntimeState, Widget, WidgetId};
use fission_ir::op::{BoxShadow, Color, Fill, LayoutOp, Op, PaintOp, Stroke, TextStyle};
use fission_ir::semantics::TextFieldValidationState;
use fission_ir::{CoreIR, PopupKind, Role, Semantics};
use fission_theme::{
    ButtonHierarchy, ComponentBorder, ComponentSize, ComponentState, ComponentStateStyles,
    ResolvedComponentStyle, ShadowLayer,
};

#[derive(Clone, Debug, PartialEq)]
struct RectPaint {
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    corner_radius: f32,
    shadow: Option<BoxShadow>,
}

fn lower(widget: Widget, env: &Env, runtime: &RuntimeState) -> CoreIR {
    let mut cx = LoweringCx::new(env, runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut cx);
    cx.ir.root = Some(root);
    cx.ir
}

fn box_geometry(ir: &CoreIR, id: WidgetId) -> (Option<f32>, Option<f32>, Option<f32>, [f32; 4]) {
    let node = ir.nodes.get(&id).expect("control layout node");
    let Op::Layout(LayoutOp::Box {
        width,
        height,
        min_height,
        padding,
        ..
    }) = &node.op
    else {
        panic!("expected control box, got {:?}", node.op);
    };
    (*width, *height, *min_height, *padding)
}

fn box_max_width(ir: &CoreIR, id: WidgetId) -> Option<f32> {
    let node = ir.nodes.get(&id).expect("control layout node");
    let Op::Layout(LayoutOp::Box { max_width, .. }) = &node.op else {
        panic!("expected control box, got {:?}", node.op);
    };
    *max_width
}

fn direct_rects(ir: &CoreIR, layout_id: WidgetId) -> Vec<RectPaint> {
    ir.nodes[&layout_id]
        .children
        .iter()
        .filter_map(|id| match &ir.nodes[id].op {
            Op::Paint(PaintOp::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
            }) => Some(RectPaint {
                fill: fill.clone(),
                stroke: stroke.clone(),
                corner_radius: *corner_radius,
                shadow: *shadow,
            }),
            _ => None,
        })
        .collect()
}

fn text_style<'a>(ir: &'a CoreIR, expected: &str) -> &'a TextStyle {
    ir.nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs
                .iter()
                .find(|run| run.text == expected)
                .map(|run| &run.style),
            _ => None,
        })
        .expect("rich text style")
}

fn shadow(color: Color, spread_radius: f32, inset: bool) -> ShadowLayer {
    ShadowLayer {
        color,
        offset: (0.0, 0.0),
        blur_radius: 0.0,
        spread_radius,
        inset,
    }
}

fn button_layout_id(ir: &CoreIR, id: WidgetId) -> WidgetId {
    match &ir.nodes[&id].op {
        Op::Semantics(_) => ir.nodes[&id].children[0],
        Op::Layout(LayoutOp::Box { .. }) => id,
        op => panic!("unexpected button root: {op:?}"),
    }
}

fn text_input_layout_id(ir: &CoreIR, id: WidgetId) -> WidgetId {
    let semantics = ir.nodes.get(&id).expect("text input semantics");
    assert!(matches!(semantics.op, Op::Semantics(_)));
    semantics.children[0]
}

#[test]
fn button_uses_compact_recipe_geometry_and_label_typography() {
    let id = WidgetId::explicit("quality.button.default");
    let env = Env::default();
    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("Save").into()),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = button_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, None, Some(32.0), [12.0, 12.0, 2.0, 2.0])
    );
    let rects = direct_rects(&ir, layout_id);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].corner_radius, 10.0);
    assert_eq!(rects[0].shadow, None);

    let label = text_style(&ir, "Save");
    assert_eq!(label.font_size, 14.0);
    assert_eq!(label.font_weight, 500);
    assert_eq!(label.line_height, Some(20.0));
}

#[test]
fn explicit_button_height_remains_an_exact_caller_constraint() {
    let id = WidgetId::explicit("quality.button.explicit-height");
    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("Fixed").into()),
            height: Some(27.0),
            ..Default::default()
        }
        .into(),
        &Env::default(),
        &RuntimeState::default(),
    );
    let layout_id = button_layout_id(&ir, id);

    let (_, height, min_height, _) = box_geometry(&ir, layout_id);
    assert_eq!(height, Some(27.0));
    assert_eq!(min_height, None);
}

#[test]
fn retained_button_content_receives_recipe_label_icon_and_gap_styles() {
    let id = WidgetId::explicit("quality.button.content-anatomy");
    let env = Env::default();
    let ir = lower(
        Button {
            id: Some(id),
            semantics: Some(Semantics {
                role: Role::Button,
                ..Semantics::default()
            }),
            child: Some(Text::new("ignored compatibility child").into()),
            content: Some(
                ButtonContent::new("Save")
                    .leading_icon(Icon::path("M0 0h8v8z"))
                    .trailing_icon(Icon::path("M0 0h8v8z").size(18.0).color(Color::RED)),
            ),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );

    let Op::Semantics(semantics) = &ir.nodes[&id].op else {
        panic!("expected button semantics root");
    };
    assert_eq!(semantics.label.as_deref(), Some("Save"));
    assert!(ir.nodes.values().all(|node| !matches!(
        &node.op,
        Op::Paint(PaintOp::DrawRichText { runs, .. })
            if runs.iter().any(|run| run.text == "ignored compatibility child")
    )));

    let content_row = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Layout(LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Row,
                gap: Some(6.0),
                ..
            }) if node.children.len() == 3 => Some(node),
            _ => None,
        })
        .expect("recipe-aware button content row");
    let icon_sizes = content_row
        .children
        .iter()
        .filter_map(|id| match &ir.nodes[id].op {
            Op::Layout(LayoutOp::Box {
                width: Some(width),
                height: Some(height),
                ..
            }) if width == height => Some(*width),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(icon_sizes, vec![16.0, 18.0]);
    let icon_fills = content_row
        .children
        .iter()
        .filter_map(|id| {
            let paint_id = ir.nodes[id].children.first()?;
            match &ir.nodes[paint_id].op {
                Op::Paint(PaintOp::DrawPath { fill, .. }) => Some(fill.clone()),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        icon_fills,
        vec![
            Some(Fill::Solid(env.theme.tokens.colors.on_primary)),
            Some(Fill::Solid(Color::RED)),
        ]
    );

    let label = text_style(&ir, "Save");
    assert_eq!(label.color, env.theme.tokens.colors.on_primary);
    assert_eq!(label.font_size, 14.0);
    assert_eq!(label.font_weight, 500);
    assert_eq!(label.line_height, Some(20.0));
}

#[test]
fn icon_only_button_receives_recipe_icon_style_and_accessible_semantics() {
    let id = WidgetId::explicit("quality.button.icon-only");
    let env = Env::default();
    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("ignored compatibility child").into()),
            content: Some(ButtonContent::new("ignored labelled content")),
            icon_content: Some(ButtonIconContent::new(
                Icon::path("M0 0h8v8z"),
                "Open options",
            )),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );

    let Op::Semantics(semantics) = &ir.nodes[&id].op else {
        panic!("expected button semantics root");
    };
    assert_eq!(semantics.role, Role::Button);
    assert_eq!(semantics.label.as_deref(), Some("Open options"));
    assert!(semantics.focusable);
    assert!(semantics.sequential_focusable);
    assert!(ir.nodes.values().all(|node| !matches!(
        &node.op,
        Op::Paint(PaintOp::DrawRichText { runs, .. })
            if runs.iter().any(|run| {
                run.text == "Open options"
                    || run.text == "ignored labelled content"
                    || run.text == "ignored compatibility child"
            })
    )));

    let icon = ir
        .nodes
        .values()
        .find(|node| {
            matches!(
                &node.op,
                Op::Layout(LayoutOp::Box {
                    width: Some(16.0),
                    height: Some(16.0),
                    ..
                })
            )
        })
        .expect("recipe-sized icon layout");
    let paint_id = icon.children.first().expect("icon paint child");
    let Op::Paint(PaintOp::DrawPath { fill, .. }) = &ir.nodes[paint_id].op else {
        panic!("expected icon path paint");
    };
    assert_eq!(fill, &Some(Fill::Solid(env.theme.tokens.colors.on_primary)));
}

#[test]
fn icon_only_button_preserves_explicit_icon_style_and_semantics_label() {
    let id = WidgetId::explicit("quality.button.icon-only-overrides");
    let ir = lower(
        Button {
            id: Some(id),
            icon_content: Some(ButtonIconContent::new(
                Icon::path("M0 0h8v8z").size(19.0).color(Color::RED),
                "Dismiss",
            )),
            semantics: Some(Semantics {
                role: Role::Button,
                label: Some("Close panel".into()),
                ..Semantics::default()
            }),
            ..Default::default()
        }
        .into(),
        &Env::default(),
        &RuntimeState::default(),
    );

    let Op::Semantics(semantics) = &ir.nodes[&id].op else {
        panic!("expected button semantics root");
    };
    assert_eq!(semantics.label.as_deref(), Some("Close panel"));

    let icon = ir
        .nodes
        .values()
        .find(|node| {
            matches!(
                &node.op,
                Op::Layout(LayoutOp::Box {
                    width: Some(19.0),
                    height: Some(19.0),
                    ..
                })
            )
        })
        .expect("explicitly sized icon layout");
    let paint_id = icon.children.first().expect("icon paint child");
    let Op::Paint(PaintOp::DrawPath { fill, .. }) = &ir.nodes[paint_id].op else {
        panic!("expected icon path paint");
    };
    assert_eq!(fill, &Some(Fill::Solid(Color::RED)));
}

#[test]
fn custom_button_recipe_overrides_legacy_geometry_and_typography_fallbacks() {
    let id = WidgetId::explicit("quality.button.custom-recipe");
    let mut env = Env::default();
    let button_theme = &mut env.theme.components.button;
    button_theme.height = 88.0;
    button_theme.padding_horizontal = 88.0;
    button_theme.padding_vertical = 88.0;
    button_theme.radius = 88.0;
    button_theme.text_size = 88.0;
    let (_, size) = button_theme
        .sizes
        .iter_mut()
        .find(|(candidate, _)| *candidate == ComponentSize::Md)
        .expect("medium button size recipe");
    *size = ResolvedComponentStyle {
        width: Some(101.0),
        max_width: Some(141.0),
        height: Some(33.0),
        padding: Some([7.0, 8.0, 3.0, 4.0]),
        radius: Some(12.0),
        font_size: Some(15.0),
        font_weight: Some(600),
        line_height: Some(21.0),
        letter_spacing: Some(0.5),
        ..ResolvedComponentStyle::default()
    };

    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("Save").into()),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = button_layout_id(&ir, id);
    assert_eq!(
        box_geometry(&ir, layout_id),
        (Some(101.0), None, Some(33.0), [9.0, 10.0, 5.0, 6.0])
    );
    assert_eq!(direct_rects(&ir, layout_id)[0].corner_radius, 12.0);
    assert_eq!(box_max_width(&ir, layout_id), Some(141.0));
    let label = text_style(&ir, "Save");
    assert_eq!(label.font_size, 15.0);
    assert_eq!(label.font_weight, 600);
    assert_eq!(label.line_height, Some(21.0));
    assert_eq!(label.letter_spacing, 0.5);
}

#[test]
fn button_style_override_is_a_real_last_mile_override() {
    let id = WidgetId::explicit("quality.button.style-override");
    let env = Env::default();
    let override_shadow = BoxShadow {
        color: Color::BLUE,
        offset: (0.0, 2.0),
        blur_radius: 5.0,
        spread_radius: 1.0,
        inset: false,
    };
    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("Override").into()),
            style: Some(ButtonStyleOverride {
                background_fill: Some(Fill::Solid(Color::GREEN)),
                text_color: Some(Color::RED),
                border: Some(Stroke {
                    fill: Fill::Solid(Color::BLACK),
                    width: 2.0,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Round,
                    line_join: fission_ir::op::LineJoin::Round,
                }),
                width: Some(121.0),
                max_width: Some(141.0),
                height: Some(37.0),
                padding: Some([11.0, 12.0, 3.0, 4.0]),
                corner_radius: Some(13.0),
                shadows: Some(vec![override_shadow]),
                font_size: Some(15.0),
                font_weight: Some(650),
                line_height: Some(22.0),
                letter_spacing: Some(0.75),
                ..Default::default()
            }),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = button_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (Some(121.0), None, Some(37.0), [13.0, 14.0, 5.0, 6.0])
    );
    assert_eq!(box_max_width(&ir, layout_id), Some(141.0));
    let rects = direct_rects(&ir, layout_id);
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].shadow, Some(override_shadow));
    assert_eq!(rects[1].fill, Some(Fill::Solid(Color::GREEN)));
    assert_eq!(
        rects[1].stroke.as_ref().map(|stroke| stroke.width),
        Some(2.0)
    );
    assert_eq!(rects[1].corner_radius, 13.0);
    let label = text_style(&ir, "Override");
    assert_eq!(label.color, Color::RED);
    assert_eq!(label.font_size, 15.0);
    assert_eq!(label.font_weight, 650);
    assert_eq!(label.line_height, Some(22.0));
    assert_eq!(label.letter_spacing, 0.75);
}

#[test]
fn replacement_component_recipe_composes_selected_hover_and_focus_without_button_leakage() {
    let id = WidgetId::explicit("quality.button.replacement-recipe");
    let mut env = Env::default();
    env.theme.components.button.height = 99.0;
    env.theme.components.button.radius = 29.0;
    env.theme.components.button.elevation_rest = Some(BoxShadow {
        color: Color::BLACK,
        offset: (0.0, 12.0),
        blur_radius: 24.0,
        spread_radius: 8.0,
        inset: false,
    });

    let focus_shadow = shadow(Color::RED, 3.0, false);
    let widget = Button {
        id: Some(id),
        child: Some(Text::new("Composite trigger").into()),
        semantics: Some(Semantics {
            role: Role::Button,
            focusable: true,
            selected: Some(true),
            ..Semantics::default()
        }),
        style: Some(ButtonStyleOverride {
            recipe_base: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(Color::GREEN)),
                text_color: Some(Color::BLACK),
                height: Some(37.0),
                radius: Some(7.0),
                padding: Some([7.0, 7.0, 4.0, 4.0]),
                ..ResolvedComponentStyle::default()
            }),
            state_styles: Some(ComponentStateStyles {
                hover: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(Color::BLUE)),
                    ..ResolvedComponentStyle::default()
                }),
                focus: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(Color::RED),
                        width: 2.0,
                    }),
                    shadows: vec![focus_shadow.clone()],
                    ..ResolvedComponentStyle::default()
                }),
                selected: Some(ResolvedComponentStyle {
                    text_color: Some(Color::WHITE),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            }),
            replace_component_recipe: true,
            ..ButtonStyleOverride::default()
        }),
        ..Button::default()
    };
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_hovered(id, true);
    runtime.interaction.set_focused(Some(id));

    let ir = lower(widget.into(), &env, &runtime);
    let layout_id = button_layout_id(&ir, id);
    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, None, Some(37.0), [9.0, 9.0, 6.0, 6.0])
    );
    let rects = direct_rects(&ir, layout_id);
    assert_eq!(
        rects.len(),
        3,
        "only recipe surface, shadow, and focus ring"
    );
    assert_eq!(rects[0].shadow, Some(focus_shadow.to_box_shadow()));
    assert_eq!(rects[1].fill, Some(Fill::Solid(Color::BLUE)));
    assert_eq!(rects[1].corner_radius, 7.0);
    assert_eq!(
        rects[2]
            .stroke
            .as_ref()
            .map(|stroke| (&stroke.fill, stroke.width)),
        Some((&Fill::Solid(Color::RED), 2.0))
    );
    assert_eq!(text_style(&ir, "Composite trigger").color, Color::WHITE);
}

#[test]
fn button_layers_focus_recipe_over_pressed_state_without_layout_shift() {
    let id = WidgetId::explicit("quality.button.focused-pressed");
    let mut env = Env::default();
    let (_, states) = env
        .theme
        .components
        .button
        .hierarchies
        .iter_mut()
        .find(|(hierarchy, _)| *hierarchy == ButtonHierarchy::Primary)
        .expect("primary button recipe");
    states.hover = Some(ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::GREEN)),
        ..ResolvedComponentStyle::default()
    });
    states.active = Some(ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::BLUE)),
        ..ResolvedComponentStyle::default()
    });
    states.focus = Some(ResolvedComponentStyle {
        border: Some(ComponentBorder {
            fill: Fill::Solid(Color::RED),
            width: 1.0,
        }),
        shadows: vec![
            shadow(Color::GREEN, 2.0, false),
            shadow(Color::BLACK, 1.0, true),
        ],
        ..ResolvedComponentStyle::default()
    });

    let widget = Button {
        id: Some(id),
        child: Some(Text::new("Save").into()),
        ..Default::default()
    };
    let default_ir = lower(widget.clone().into(), &env, &RuntimeState::default());
    let mut hovered_runtime = RuntimeState::default();
    hovered_runtime.interaction.set_hovered(id, true);
    let hovered_ir = lower(widget.clone().into(), &env, &hovered_runtime);
    let hovered_layout_id = button_layout_id(&hovered_ir, id);
    assert_eq!(
        direct_rects(&hovered_ir, hovered_layout_id)
            .last()
            .and_then(|paint| paint.fill.clone()),
        Some(Fill::Solid(Color::GREEN))
    );
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_pressed(id, true);
    runtime.interaction.set_focused(Some(id));
    let focused_ir = lower(widget.into(), &env, &runtime);
    let default_layout_id = button_layout_id(&default_ir, id);
    let focused_layout_id = button_layout_id(&focused_ir, id);

    assert_eq!(
        box_geometry(&focused_ir, focused_layout_id),
        box_geometry(&default_ir, default_layout_id),
        "paint-only interaction states must not change button geometry"
    );
    let rects = direct_rects(&focused_ir, focused_layout_id);
    assert_eq!(rects.len(), 4);
    assert_eq!(
        rects[0].shadow,
        Some(shadow(Color::GREEN, 2.0, false).to_box_shadow())
    );
    assert_eq!(rects[1].fill, Some(Fill::Solid(Color::BLUE)));
    assert_eq!(
        rects[1].stroke.as_ref().map(|stroke| &stroke.fill),
        Some(&Fill::Solid(Color::TRANSPARENT))
    );
    assert_eq!(rects[1].shadow, None);
    assert_eq!(
        rects[2].shadow,
        Some(shadow(Color::BLACK, 1.0, true).to_box_shadow())
    );
    assert_eq!(
        rects[3].stroke.as_ref().map(|stroke| &stroke.fill),
        Some(&Fill::Solid(Color::RED))
    );
}

#[test]
fn disabled_button_recipe_wins_over_all_runtime_interaction() {
    let id = WidgetId::explicit("quality.button.disabled");
    let mut env = Env::default();
    env.theme.components.button.elevation_rest =
        Some(shadow(Color::RED, 2.0, false).to_box_shadow());
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_hovered(id, true);
    runtime.interaction.set_pressed(id, true);
    runtime.interaction.set_focused(Some(id));
    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("Save").into()),
            disabled: true,
            ..Default::default()
        }
        .into(),
        &env,
        &runtime,
    );
    let layout_id = button_layout_id(&ir, id);
    let rects = direct_rects(&ir, layout_id);

    assert_eq!(
        rects.len(),
        1,
        "disabled buttons must not paint focus rings"
    );
    let disabled_style = env.theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Disabled,
    );
    assert_eq!(rects[0].fill, disabled_style.background);
    assert_eq!(
        text_style(&ir, "Save").color,
        disabled_style.text_color.unwrap()
    );
    let opacity = ir.nodes[&layout_id]
        .composite
        .opacity
        .as_ref()
        .expect("whole-button disabled opacity");
    assert_eq!(opacity.base, 0.5);
}

#[test]
fn active_button_translates_the_complete_control_by_one_point() {
    let id = WidgetId::explicit("quality.button.active-offset");
    let env = Env::default();
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_pressed(id, true);
    let ir = lower(
        Button {
            id: Some(id),
            child: Some(Text::new("Save").into()),
            ..Default::default()
        }
        .into(),
        &env,
        &runtime,
    );
    let layout_id = button_layout_id(&ir, id);
    let translate_y = ir.nodes[&layout_id]
        .composite
        .translate_y
        .as_ref()
        .expect("whole-button active translation");
    assert_eq!(translate_y.base, 1.0);
}

#[test]
fn outline_and_neutral_secondary_keep_distinct_surface_recipes() {
    let env = Env::default();
    let outline_id = WidgetId::explicit("quality.button.outline");
    let secondary_id = WidgetId::explicit("quality.button.secondary");
    let outline = lower(
        Button {
            id: Some(outline_id),
            child: Some(Text::new("Outline").into()),
            variant: ButtonVariant::Outline,
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let secondary = lower(
        Button {
            id: Some(secondary_id),
            child: Some(Text::new("Secondary").into()),
            variant: ButtonVariant::SecondaryGray,
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let outline_surface = direct_rects(&outline, button_layout_id(&outline, outline_id));
    let secondary_surface = direct_rects(&secondary, button_layout_id(&secondary, secondary_id));

    assert_ne!(
        (
            &outline_surface[0].fill,
            outline_surface[0]
                .stroke
                .as_ref()
                .map(|stroke| &stroke.fill),
        ),
        (
            &secondary_surface[0].fill,
            secondary_surface[0]
                .stroke
                .as_ref()
                .map(|stroke| &stroke.fill),
        )
    );
}

#[test]
fn every_button_hierarchy_gets_a_focus_ring_with_destructive_colour_override() {
    let env = Env::default();
    for (variant, expected) in [
        (ButtonVariant::Outline, env.theme.tokens.colors.focus_ring),
        (
            ButtonVariant::SecondaryGray,
            env.theme.tokens.colors.focus_ring,
        ),
        (ButtonVariant::Ghost, env.theme.tokens.colors.focus_ring),
        (ButtonVariant::Destructive, env.theme.tokens.colors.error),
    ] {
        let id = WidgetId::explicit(&format!("quality.button.focus.{variant:?}"));
        let mut runtime = RuntimeState::default();
        runtime.interaction.set_focused(Some(id));
        let ir = lower(
            Button {
                id: Some(id),
                child: Some(Text::new("Focus").into()),
                variant,
                ..Default::default()
            }
            .into(),
            &env,
            &runtime,
        );
        let rects = direct_rects(&ir, button_layout_id(&ir, id));
        let ring = rects
            .iter()
            .rev()
            .find_map(|paint| paint.stroke.as_ref())
            .expect("focus ring stroke");
        assert_eq!(ring.fill, Fill::Solid(expected), "{variant:?}");
    }
}

#[test]
fn text_input_uses_compact_recipe_geometry_typography_and_border() {
    let id = WidgetId::explicit("quality.input.default");
    let env = Env::default();
    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = text_input_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, Some(32.0), Some(32.0), [11.0, 11.0, 5.0, 5.0])
    );
    let rects = direct_rects(&ir, layout_id);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].corner_radius, 10.0);
    assert_eq!(
        rects[0].stroke.as_ref().map(|stroke| stroke.width),
        Some(1.0)
    );
    assert_eq!(rects[0].shadow, None);

    let text = text_style(&ir, "Ada");
    assert_eq!(text.font_size, 14.0);
    assert_eq!(text.font_weight, 400);
    assert_eq!(text.line_height, Some(20.0));
}

#[test]
fn text_input_uses_touch_safe_typography_below_the_narrow_breakpoint() {
    let id = WidgetId::explicit("quality.input.narrow-typography");
    let env = Env {
        viewport_size: LayoutSize::new(767.0, 900.0),
        ..Env::default()
    };
    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = text_input_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, Some(32.0), Some(32.0), [11.0, 11.0, 5.0, 5.0])
    );
    let text = text_style(&ir, "Ada");
    assert_eq!(text.font_size, 16.0);
    assert_eq!(text.line_height, Some(20.0));
}

#[test]
fn small_text_input_recipe_fits_its_line_box_without_changing_height() {
    let id = WidgetId::explicit("quality.input.small-geometry");
    let env = Env::default();
    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            size: ComponentSize::Sm,
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = text_input_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, Some(28.0), Some(28.0), [11.0, 11.0, 4.0, 4.0])
    );
    assert_eq!(text_style(&ir, "Ada").line_height, Some(20.0));
}

#[test]
fn implicit_text_input_height_grows_with_text_scale_to_avoid_clipping() {
    let id = WidgetId::explicit("quality.input.scaled-geometry");
    let env = Env::default();
    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            text_scale: Some(1.5),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = text_input_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, Some(40.0), Some(40.0), [11.0, 11.0, 5.0, 5.0])
    );
    let text = text_style(&ir, "Ada");
    assert_eq!(text.font_size, 21.0);
    assert_eq!(text.line_height, Some(30.0));
}

#[test]
fn explicit_text_input_height_remains_authoritative_over_scaled_content() {
    let id = WidgetId::explicit("quality.input.explicit-height");
    let env = Env::default();
    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            height: Some(33.0),
            text_scale: Some(1.5),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = text_input_layout_id(&ir, id);

    assert_eq!(
        box_geometry(&ir, layout_id),
        (None, Some(33.0), None, [11.0, 11.0, 5.0, 5.0])
    );
}

#[test]
fn explicit_text_input_typography_wins_at_narrow_widths() {
    let id = WidgetId::explicit("quality.input.narrow-explicit-typography");
    let env = Env {
        viewport_size: LayoutSize::new(320.0, 640.0),
        ..Env::default()
    };
    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            font_size: Some(18.0),
            line_height: Some(26.0),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let text = text_style(&ir, "Ada");
    assert_eq!(text.font_size, 18.0);
    assert_eq!(text.line_height, Some(26.0));
}

#[test]
fn text_input_placeholder_and_helper_use_secondary_text_tokens() {
    let id = WidgetId::explicit("quality.input.supporting-colour");
    let env = Env::default();
    let ir = lower(
        TextInput {
            id: Some(id),
            value: String::new(),
            placeholder: Some("Search".into()),
            helper_text: Some("Optional".into()),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );

    assert_eq!(
        text_style(&ir, "Search").color,
        env.theme.tokens.colors.text_secondary
    );
    assert_eq!(
        text_style(&ir, "Optional").color,
        env.theme.tokens.colors.text_secondary
    );
}

#[test]
fn dark_text_input_uses_recipe_fill_and_error_ring() {
    let id = WidgetId::explicit("quality.input.dark-error");
    let env = Env {
        theme: fission_theme::Theme::dark(),
        ..Env::default()
    };
    let default_ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let default_rects = direct_rects(&default_ir, text_input_layout_id(&default_ir, id));
    let expected_default = env
        .theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Default);
    assert_eq!(default_rects[0].fill, expected_default.background);
    assert!(matches!(
        default_rects[0].fill.as_ref(),
        Some(Fill::Solid(color)) if color.a > 0
    ));

    let error_ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            validation_state: TextFieldValidationState::Invalid,
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let error_rects = direct_rects(&error_ir, text_input_layout_id(&error_ir, id));
    let expected_error = env
        .theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Error);
    assert_eq!(
        error_rects[0].shadow,
        expected_error
            .shadows
            .first()
            .map(ShadowLayer::to_box_shadow)
    );
    assert_eq!(
        error_rects[1].stroke.as_ref().map(|stroke| &stroke.fill),
        expected_error.border.as_ref().map(|border| &border.fill)
    );
}

#[test]
fn custom_text_input_recipe_overrides_legacy_geometry_and_typography_fallbacks() {
    let id = WidgetId::explicit("quality.input.custom-recipe");
    let mut env = Env::default();
    let input_theme = &mut env.theme.components.text_input;
    input_theme.height = 88.0;
    input_theme.padding_h = 88.0;
    input_theme.radius = 88.0;
    input_theme.font_size = 88.0;
    input_theme.border_width = 8.0;
    let (_, size) = input_theme
        .sizes
        .iter_mut()
        .find(|(candidate, _)| *candidate == ComponentSize::Md)
        .expect("medium input size recipe");
    *size = ResolvedComponentStyle {
        width: Some(177.0),
        max_width: Some(217.0),
        height: Some(34.0),
        padding: Some([9.0, 8.0, 6.0, 5.0]),
        radius: Some(12.0),
        font_size: Some(15.0),
        font_weight: Some(550),
        line_height: Some(21.0),
        letter_spacing: Some(0.4),
        ..ResolvedComponentStyle::default()
    };
    input_theme.states.default.background = Some(Fill::Solid(Color::BLUE));
    input_theme.states.default.border = Some(ComponentBorder {
        fill: Fill::Solid(Color::RED),
        width: 1.0,
    });

    let ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            ..Default::default()
        }
        .into(),
        &env,
        &RuntimeState::default(),
    );
    let layout_id = text_input_layout_id(&ir, id);
    assert_eq!(
        box_geometry(&ir, layout_id),
        (Some(177.0), Some(34.0), Some(34.0), [10.0, 9.0, 7.0, 6.0])
    );
    let rects = direct_rects(&ir, layout_id);
    assert_eq!(box_max_width(&ir, layout_id), Some(217.0));
    let rect = &rects[0];
    assert_eq!(rect.fill, Some(Fill::Solid(Color::BLUE)));
    assert_eq!(rect.corner_radius, 12.0);
    let stroke = rect.stroke.as_ref().expect("input recipe border");
    assert_eq!(stroke.fill, Fill::Solid(Color::RED));
    assert_eq!(stroke.width, 1.0);
    let text = text_style(&ir, "Ada");
    assert_eq!(text.font_size, 15.0);
    assert_eq!(text.font_weight, 550);
    assert_eq!(text.line_height, Some(21.0));
    assert_eq!(text.letter_spacing, 0.4);
}

#[test]
fn text_input_preserves_every_focus_shadow_layer_without_layout_shift() {
    let id = WidgetId::explicit("quality.input.focus");
    let mut env = Env::default();
    env.theme.components.text_input.states.focus = Some(ResolvedComponentStyle {
        border: Some(ComponentBorder {
            fill: Fill::Solid(Color::RED),
            width: 1.0,
        }),
        shadows: vec![
            shadow(Color::GREEN, 3.0, false),
            shadow(Color::BLACK, 1.0, true),
        ],
        ..ResolvedComponentStyle::default()
    });
    let widget = TextInput {
        id: Some(id),
        value: "Ada".into(),
        ..Default::default()
    };
    let default_ir = lower(widget.clone().into(), &env, &RuntimeState::default());
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_focused(Some(id));
    let focused_ir = lower(widget.into(), &env, &runtime);
    let default_layout_id = text_input_layout_id(&default_ir, id);
    let focused_layout_id = text_input_layout_id(&focused_ir, id);

    assert_eq!(
        box_geometry(&focused_ir, focused_layout_id),
        box_geometry(&default_ir, default_layout_id),
        "focus paint must not change text-input geometry"
    );
    let rects = direct_rects(&focused_ir, focused_layout_id);
    assert_eq!(rects.len(), 3);
    assert_eq!(
        rects[0].shadow,
        Some(shadow(Color::GREEN, 3.0, false).to_box_shadow())
    );
    assert_eq!(
        rects[1].stroke.as_ref().map(|stroke| &stroke.fill),
        Some(&Fill::Solid(Color::RED))
    );
    assert_eq!(rects[1].shadow, None);
    assert_eq!(
        rects[2].shadow,
        Some(shadow(Color::BLACK, 1.0, true).to_box_shadow())
    );
}

#[test]
fn text_input_resolves_hover_invalid_and_disabled_recipes_in_priority_order() {
    let id = WidgetId::explicit("quality.input.states");
    let mut env = Env::default();
    env.theme.components.text_input.states.hover = Some(ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::BLUE)),
        ..ResolvedComponentStyle::default()
    });
    env.theme.components.text_input.states.error = Some(ResolvedComponentStyle {
        border: Some(ComponentBorder {
            fill: Fill::Solid(Color::RED),
            width: 1.0,
        }),
        shadows: vec![shadow(Color::RED, 3.0, false)],
        ..ResolvedComponentStyle::default()
    });
    env.theme.components.text_input.states.disabled = Some(ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::BLACK)),
        border: Some(ComponentBorder {
            fill: Fill::Solid(Color::GREEN),
            width: 1.0,
        }),
        ..ResolvedComponentStyle::default()
    });

    let mut hovered = RuntimeState::default();
    hovered.interaction.set_hovered(id, true);
    let hover_ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            ..Default::default()
        }
        .into(),
        &env,
        &hovered,
    );
    let hover_rects = direct_rects(&hover_ir, text_input_layout_id(&hover_ir, id));
    assert_eq!(hover_rects[0].fill, Some(Fill::Solid(Color::BLUE)));

    let mut focused = hovered.clone();
    focused.interaction.set_focused(Some(id));
    let invalid_ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            validation_state: TextFieldValidationState::Invalid,
            ..Default::default()
        }
        .into(),
        &env,
        &focused,
    );
    let invalid_rects = direct_rects(&invalid_ir, text_input_layout_id(&invalid_ir, id));
    assert_eq!(invalid_rects.len(), 2);
    assert_eq!(
        invalid_rects[0].shadow,
        Some(shadow(Color::RED, 3.0, false).to_box_shadow())
    );
    assert_eq!(
        invalid_rects[1].stroke.as_ref().map(|stroke| &stroke.fill),
        Some(&Fill::Solid(Color::RED))
    );

    let disabled_ir = lower(
        TextInput {
            id: Some(id),
            value: "Ada".into(),
            validation_state: TextFieldValidationState::Invalid,
            enabled: false,
            ..Default::default()
        }
        .into(),
        &env,
        &focused,
    );
    let disabled_rects = direct_rects(&disabled_ir, text_input_layout_id(&disabled_ir, id));
    assert_eq!(disabled_rects.len(), 1);
    assert_eq!(disabled_rects[0].fill, Some(Fill::Solid(Color::BLACK)));
    assert_eq!(
        disabled_rects[0].stroke.as_ref().map(|stroke| &stroke.fill),
        Some(&Fill::Solid(Color::GREEN))
    );
}

#[test]
fn text_input_merges_composite_relationships_while_owning_editing_semantics() {
    let id = WidgetId::explicit("quality.input.composite");
    let label_id = WidgetId::explicit("quality.input.label");
    let description_id = WidgetId::explicit("quality.input.description");
    let popup_id = WidgetId::explicit("quality.input.popup");
    let option_id = WidgetId::explicit("quality.input.option");
    let ir = lower(
        TextInput {
            id: Some(id),
            value: String::new(),
            semantics: Some(Semantics {
                role: Role::ComboBox,
                label: Some("Project".into()),
                identifier: Some("project-picker".into()),
                actions: fission_ir::ActionSet {
                    entries: vec![fission_ir::ActionEntry {
                        trigger: fission_ir::ActionTrigger::Dismiss,
                        action_id: 17,
                        payload_data: None,
                    }],
                },
                focusable: false,
                sequential_focusable: false,
                expanded: Some(true),
                has_popup: Some(PopupKind::ListBox),
                controls: vec![popup_id],
                labelled_by: vec![label_id],
                described_by: vec![description_id],
                active_descendant: Some(option_id),
                required: true,
                validation_message: Some("Choose a project".into()),
                ..Semantics::default()
            }),
            ..Default::default()
        }
        .into(),
        &Env::default(),
        &RuntimeState::default(),
    );
    let Op::Semantics(semantics) = &ir.nodes[&id].op else {
        panic!("expected editable semantics root");
    };

    assert_eq!(semantics.role, Role::ComboBox);
    assert!(semantics.text_editable);
    assert!(semantics.supports_text_editing());
    assert_eq!(semantics.label.as_deref(), Some("Project"));
    assert_eq!(semantics.identifier.as_deref(), Some("project-picker"));
    assert!(semantics.focusable);
    assert!(semantics.sequential_focusable);
    assert_eq!(semantics.expanded, Some(true));
    assert_eq!(semantics.has_popup, Some(PopupKind::ListBox));
    assert_eq!(semantics.controls, vec![popup_id]);
    assert_eq!(semantics.labelled_by, vec![label_id]);
    assert_eq!(semantics.described_by, vec![description_id]);
    assert_eq!(semantics.active_descendant, Some(option_id));
    assert!(semantics.actions.entries.iter().any(|entry| {
        entry.trigger == fission_ir::ActionTrigger::Dismiss && entry.action_id == 17
    }));
    assert!(semantics.required);
    assert_eq!(
        semantics.validation_state,
        TextFieldValidationState::Invalid
    );
    assert_eq!(
        semantics.validation_message.as_deref(),
        Some("Choose a project")
    );
    assert_eq!(semantics.value.as_deref(), Some(""));
}
