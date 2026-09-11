use fission_core::authoring::BuildCtx;
use fission_core::op::{Fill, JustifyContent, Length};
use fission_core::ui::{CardPattern, ComponentSize, Container, Text};
use fission_core::{
    build, Env, GlobalState, LayoutDirection, RuntimeState, View, Widget, WidgetId, WidgetIdExt,
};
use fission_ir::{CoreIR, LayoutOp, Op, PaintOp};
use fission_theme::ComponentBorder;
use fission_widgets::{
    Card, CardContent, CardDescription, CardFooter, CardHeader, CardLayout, CardTitle,
};

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn lower(env: &Env, build_widget: impl FnOnce() -> Widget) -> CoreIR {
    let state = TestState;
    let runtime = RuntimeState::default();
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    fission_core::internal::lower_widget_to_ir(&widget)
}

fn rich_text_style(
    ir: &CoreIR,
    expected_text: &str,
) -> (f32, u16, Option<f32>, fission_ir::op::Color) {
    ir.nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs
                .iter()
                .find(|run| run.text == expected_text)
                .map(|run| {
                    (
                        run.style.font_size,
                        run.style.font_weight,
                        run.style.line_height,
                        run.style.color,
                    )
                }),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing rich text {expected_text:?}"))
}

fn contains_text(ir: &CoreIR, expected_text: &str) -> bool {
    ir.nodes.values().any(|node| match &node.op {
        Op::Paint(PaintOp::DrawText { text, .. }) => text == expected_text,
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            runs.iter().any(|run| run.text == expected_text)
        }
        _ => false,
    })
}

#[test]
fn sectioned_card_uses_edge_to_edge_separators_and_region_padding() {
    let mut env = Env::default();
    env.theme
        .components
        .card
        .sizes
        .iter_mut()
        .find(|(size, _)| *size == ComponentSize::Md)
        .expect("medium card recipe")
        .1
        .padding = Some([23.0; 4]);
    let card_id = WidgetId::explicit("card.sections");
    let ir = lower(&env, || {
        CardLayout::new()
            .header(CardHeader::new(CardTitle::new("Profile")))
            .content(CardContent::new(Text::new("Body")))
            .footer(CardFooter::new(vec![Text::new("Save").into()]))
            .pattern(CardPattern::Plain)
            .id(card_id)
    });

    let root = &ir.nodes[&card_id];
    match &root.op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(style.padding, None, "the surface must not inset dividers");
        }
        op => panic!("expected card surface, got {op:?}"),
    }

    let expected_padding = Some(Length::all(Length::points(23.0)));
    let padded_regions = ir
        .nodes
        .values()
        .filter(|node| match &node.op {
            Op::Layout(LayoutOp::StyledBox { style, .. }) => style.padding == expected_padding,
            _ => false,
        })
        .count();
    assert_eq!(padded_regions, 3, "header, content, and footer own padding");

    let separators = ir
        .nodes
        .values()
        .filter(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                stroke: None,
                corner_radius,
                shadow: None,
            }) => *color == env.theme.tokens.colors.border && *corner_radius == 0.0,
            _ => false,
        })
        .count();
    assert_eq!(separators, 2, "adjacent regions need one separator each");
}

#[test]
fn card_anatomy_preserves_custom_title_content_and_actions() {
    let env = Env::default();
    let ir = lower(&env, || {
        CardLayout::new()
            .header(
                CardHeader::custom(Container::new(Text::new("Custom heading")))
                    .custom_description(Container::new(Text::new("Custom description")))
                    .action(Container::new(Text::new("Header action"))),
            )
            .content(CardContent::new(Container::new(Text::new("Custom body"))))
            .footer(CardFooter::new(vec![
                Container::new(Text::new("Cancel action")).into(),
                Container::new(Text::new("Save action")).into(),
            ]))
            .into()
    });

    for expected in [
        "Custom heading",
        "Custom description",
        "Header action",
        "Custom body",
        "Cancel action",
        "Save action",
    ] {
        assert!(
            contains_text(&ir, expected),
            "missing custom slot {expected:?}"
        );
    }

    assert!(
        ir.nodes.values().any(|node| match &node.op {
            Op::Layout(LayoutOp::Flex {
                justify_content, ..
            }) => *justify_content == JustifyContent::End,
            _ => false,
        }),
        "the standard footer must end-align its action children"
    );
}

#[test]
fn standard_card_heading_text_uses_active_typography_tokens() {
    let mut env = Env::default();
    let card_theme = &mut env.theme.components.card;
    let medium = &mut card_theme
        .sizes
        .iter_mut()
        .find(|(size, _)| *size == ComponentSize::Md)
        .expect("medium card recipe")
        .1;
    medium.font_size = Some(17.0);
    medium.line_height = Some(21.25);
    card_theme.title_style.font_weight = Some(530);
    card_theme.description_style.font_size = Some(17.0);
    card_theme.description_style.font_weight = Some(410);
    card_theme.description_style.line_height = Some(21.25);
    let ir = lower(&env, || {
        CardLayout::new()
            .header(
                CardHeader::new(CardTitle::new("Token title"))
                    .description(CardDescription::new("Token description")),
            )
            .into()
    });

    let title = rich_text_style(&ir, "Token title");
    assert_eq!(title.0, 17.0);
    assert_eq!(title.1, 530);
    assert_eq!(title.2, Some(21.25));
    assert_eq!(title.3, env.theme.tokens.colors.text_primary);

    let description = rich_text_style(&ir, "Token description");
    assert_eq!(description.0, 17.0);
    assert_eq!(description.1, 410);
    assert_eq!(description.2, Some(21.25));
    assert_eq!(description.3, env.theme.tokens.colors.text_secondary);
}

#[test]
fn legacy_card_keeps_resolved_content_padding_and_single_child_contract() {
    let mut env = Env::default();
    let plain = &mut env
        .theme
        .components
        .card
        .patterns
        .iter_mut()
        .find(|(pattern, _)| *pattern == CardPattern::Plain)
        .expect("plain card recipe")
        .1;
    plain.padding_x = Some(19.0);
    plain.padding_y = Some(19.0);
    let card_id = WidgetId::explicit("card.legacy");
    let ir = lower(&env, || {
        Card {
            child: Text::new("Legacy body").into(),
            pattern: CardPattern::Plain,
            interactive: false,
            selected: false,
        }
        .id(card_id)
    });

    match &ir.nodes[&card_id].op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(style.padding, Some(Length::all(Length::points(19.0))));
        }
        op => panic!("expected legacy card surface, got {op:?}"),
    }
    assert!(contains_text(&ir, "Legacy body"));
}

#[test]
fn section_separators_are_optional_without_changing_region_spacing() {
    let env = Env::default();
    let card_id = WidgetId::explicit("card.unseparated");
    let ir = lower(&env, || {
        CardLayout::new()
            .header(CardHeader::new(CardTitle::new("Header")))
            .content(CardContent::new(Text::new("Content")))
            .separated(false)
            .id(card_id)
    });

    let separators = ir
        .nodes
        .values()
        .filter(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                stroke: None,
                corner_radius,
                shadow: None,
            }) => *color == env.theme.tokens.colors.border && *corner_radius == 0.0,
            _ => false,
        })
        .count();
    assert_eq!(separators, 0);

    match &ir.nodes[&card_id].op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(
                style.padding,
                Some([
                    Length::points(0.0),
                    Length::points(0.0),
                    Length::points(16.0),
                    Length::points(16.0),
                ])
            );
        }
        op => panic!("expected card surface, got {op:?}"),
    }

    let expected_padding = Some([
        Length::points(16.0),
        Length::points(16.0),
        Length::points(0.0),
        Length::points(0.0),
    ]);
    assert_eq!(
        ir.nodes
            .values()
            .filter(|node| match &node.op {
                Op::Layout(LayoutOp::StyledBox { style, .. }) => {
                    style.padding == expected_padding
                }
                _ => false,
            })
            .count(),
        2
    );
    assert!(ir.nodes.values().any(|node| match &node.op {
        Op::Layout(LayoutOp::Flex { gap, .. }) => *gap == Some(16.0),
        _ => false,
    }));
}

#[test]
fn card_layout_density_controls_region_padding_spacing_and_title_type() {
    assert_eq!(CardLayout::default().size, ComponentSize::Md);

    let env = Env::default();
    let small = lower(&env, || {
        CardLayout::new()
            .header(CardHeader::new(CardTitle::new("Small title")))
            .content(CardContent::new(Text::new("Small content")))
            .size(ComponentSize::Sm)
            .into()
    });

    let small_padding = Some(Length::all(Length::points(12.0)));
    assert_eq!(
        small
            .nodes
            .values()
            .filter(|node| match &node.op {
                Op::Layout(LayoutOp::StyledBox { style, .. }) => {
                    style.padding == small_padding
                }
                _ => false,
            })
            .count(),
        2
    );
    let small_title = rich_text_style(&small, "Small title");
    assert_eq!(small_title.0, 14.0);
    assert_eq!(small_title.2, Some(20.0));

    let medium = lower(&env, || {
        CardLayout::new()
            .header(CardHeader::new(CardTitle::new("Medium title")))
            .into()
    });
    let medium_title = rich_text_style(&medium, "Medium title");
    assert_eq!(medium_title.0, 16.0);
    assert_eq!(medium_title.2, Some(24.0));
}

#[test]
fn footer_uses_theme_tint_gap_and_full_width_separator() {
    let mut env = Env::default();
    let footer_tint = fission_ir::op::Color {
        r: 11,
        g: 22,
        b: 33,
        a: 255,
    };
    let separator_tint = fission_ir::op::Color {
        r: 44,
        g: 55,
        b: 66,
        a: 255,
    };
    env.theme.components.card.footer_style.background = Some(Fill::Solid(footer_tint));
    env.theme.components.card.footer_style.gap = Some(11.0);
    let separator = env
        .theme
        .components
        .card
        .footer_style
        .border
        .as_mut()
        .expect("footer boundary recipe");
    separator.fill = Fill::Solid(separator_tint);
    separator.width = 2.0;
    let ir = lower(&env, || {
        CardLayout::new()
            .content(CardContent::new(Text::new("Body")))
            .footer(CardFooter::new(vec![
                Text::new("Cancel").into(),
                Text::new("Save").into(),
            ]))
            .pattern(CardPattern::Plain)
            .into()
    });

    assert!(ir.nodes.values().any(|node| match &node.op {
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(color)),
            ..
        }) => *color == footer_tint,
        _ => false,
    }));
    assert!(ir.nodes.values().any(|node| match &node.op {
        Op::Layout(LayoutOp::Flex { gap, .. }) => *gap == Some(11.0),
        _ => false,
    }));

    let separators = ir
        .nodes
        .values()
        .filter(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                stroke: None,
                corner_radius,
                shadow: None,
            }) => *color == separator_tint && *corner_radius == 0.0,
            _ => false,
        })
        .count();
    assert_eq!(separators, 1);
    assert!(
        !ir.nodes.values().any(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                stroke: Some(stroke),
                ..
            }) => stroke.fill == Fill::Solid(separator_tint),
            _ => false,
        }),
        "the footer recipe border is a top boundary, not a four-sided stroke"
    );
}

#[test]
fn separated_footer_falls_back_to_the_shared_section_boundary_recipe() {
    let mut env = Env::default();
    let fallback_tint = fission_ir::op::Color {
        r: 73,
        g: 101,
        b: 129,
        a: 255,
    };
    env.theme.components.card.footer_style.border = None;
    let separator = env
        .theme
        .components
        .card
        .separator_style
        .border
        .as_mut()
        .expect("section boundary recipe");
    separator.fill = Fill::Solid(fallback_tint);
    separator.width = 2.5;

    let ir = lower(&env, || {
        CardLayout::new()
            .content(CardContent::new(Text::new("Body")))
            .footer(CardFooter::new(vec![Text::new("Save").into()]))
            .pattern(CardPattern::Plain)
            .into()
    });

    assert_eq!(
        ir.nodes
            .values()
            .filter(|node| match &node.op {
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill::Solid(color)),
                    stroke: None,
                    corner_radius,
                    shadow: None,
                }) => *color == fallback_tint && *corner_radius == 0.0,
                _ => false,
            })
            .count(),
        1
    );
}

#[test]
fn card_layout_supports_controlled_selection_and_inset_separators() {
    let selected_tint = fission_ir::op::Color {
        r: 19,
        g: 83,
        b: 211,
        a: 255,
    };
    let mut env = Env::default();
    env.theme.components.card.selected_style.border = Some(ComponentBorder {
        fill: Fill::Solid(selected_tint),
        width: 2.0,
    });
    let card_id = WidgetId::explicit("card.selected-inset");
    let ir = lower(&env, || {
        CardLayout::new()
            .header(CardHeader::new(CardTitle::new("Selected")))
            .content(CardContent::new(Text::new("Body")))
            .selected(true)
            .separator_inset(16.0)
            .id(card_id)
    });

    assert!(ir.nodes.values().any(|node| {
        matches!(
            &node.op,
            Op::Paint(PaintOp::DrawRect {
                stroke: Some(stroke),
                ..
            }) if stroke.fill == Fill::Solid(selected_tint) && stroke.width == 2.0
        )
    }));

    assert!(ir.nodes.values().any(|node| match &node.op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            style.padding
                == Some([
                    Length::points(16.0),
                    Length::points(16.0),
                    Length::points(0.0),
                    Length::points(0.0),
                ])
        }
        _ => false,
    }));
}

#[test]
fn selected_card_indicator_uses_the_logical_leading_edge() {
    let accent = fission_ir::op::Color {
        r: 19,
        g: 83,
        b: 211,
        a: 255,
    };
    for (direction, expected_left, expected_right) in [
        (LayoutDirection::LeftToRight, Some(0.0), None),
        (LayoutDirection::RightToLeft, None, Some(0.0)),
    ] {
        let mut env = Env::default();
        env.layout_direction = direction;
        env.theme.components.card.selected_indicator_style.width = Some(5.0);
        env.theme
            .components
            .card
            .selected_indicator_style
            .background = Some(Fill::Solid(accent));
        env.theme.components.card.selected_indicator_style.margin = Some([0.0, 0.0, 3.0, 7.0]);

        let ir = lower(&env, || {
            CardLayout::new()
                .content(CardContent::new(Text::new("Selected")))
                .selected(true)
                .into()
        });
        assert!(ir.nodes.values().any(|node| {
            matches!(
                node.op,
                Op::Layout(LayoutOp::Positioned {
                    left,
                    right,
                    top: Some(3.0),
                    bottom: Some(7.0),
                    width: Some(5.0),
                    ..
                }) if left == expected_left && right == expected_right
            )
        }));
        assert!(ir.nodes.values().any(|node| {
            matches!(
                &node.op,
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill::Solid(color)),
                    ..
                }) if *color == accent
            )
        }));
    }
}

#[test]
fn card_indicator_is_absent_when_unselected_or_disabled_by_recipe() {
    for (selected, width) in [(false, Some(4.0)), (true, None)] {
        let mut env = Env::default();
        env.theme.components.card.selected_indicator_style.width = width;
        let ir = lower(&env, || {
            Card {
                child: Text::new("Card").into(),
                selected,
                ..Default::default()
            }
            .into()
        });
        assert!(!ir.nodes.values().any(|node| {
            matches!(
                node.op,
                Op::Layout(LayoutOp::Positioned {
                    width: Some(4.0),
                    ..
                })
            )
        }));
    }
}

#[test]
fn card_separator_uses_recipe_margin_when_layout_has_no_override() {
    let mut env = Env::default();
    env.theme.components.card.separator_style.margin = Some([12.0, 13.0, 2.0, 3.0]);
    let ir = lower(&env, || {
        CardLayout::new()
            .header(CardHeader::new(CardTitle::new("Header")))
            .content(CardContent::new(Text::new("Body")))
            .into()
    });

    assert!(ir.nodes.values().any(|node| match &node.op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            style.padding
                == Some([
                    Length::points(12.0),
                    Length::points(13.0),
                    Length::points(2.0),
                    Length::points(3.0),
                ])
        }
        _ => false,
    }));
}
