use super::{
    flyout_root_position, resolve_length, resolve_position_length, LayoutEngine, LayoutGraphState,
    LayoutInputNode, LayoutPoint, LayoutRect, LayoutSize, TextMeasurer,
    DEFAULT_RICH_TEXT_HIT_TEST_FONT_SIZE,
};
use fission_ir::op::{
    BoxStyle, Color, FontStyle, GridTrack, Length, ResponsiveCondition, ResponsiveQuery, TextRun,
    TextStyle,
};
use fission_ir::{GridPlacement, LayoutOp, WidgetId};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn box_node(
    id: WidgetId,
    parent_id: Option<WidgetId>,
    children_ids: Vec<WidgetId>,
) -> LayoutInputNode {
    LayoutInputNode {
        id,
        parent_id,
        op: LayoutOp::Box {
            width: Some(40.0),
            height: Some(20.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        },
        children_ids,
        debug_name: format!("node-{}", id.as_u128()),
        width: Some(40.0),
        height: Some(20.0),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    }
}

struct RecordingMeasurer {
    last_font_size_bits: AtomicU32,
}

struct WrappingMeasurer;

impl TextMeasurer for WrappingMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let natural_width = text.chars().count() as f32 * 10.0;
        match available_width.filter(|width| *width > 0.0 && natural_width > *width) {
            Some(width) => (width, (natural_width / width).ceil() * 20.0),
            None => (natural_width, 20.0),
        }
    }
}

fn node(
    id: WidgetId,
    parent_id: Option<WidgetId>,
    children_ids: Vec<WidgetId>,
    op: LayoutOp,
) -> LayoutInputNode {
    let (width, height, flex_grow, flex_shrink) = match &op {
        LayoutOp::Box {
            width,
            height,
            flex_grow,
            flex_shrink,
            ..
        } => (*width, *height, *flex_grow, *flex_shrink),
        LayoutOp::StyledBox {
            flex_grow,
            flex_shrink,
            ..
        } => (None, None, *flex_grow, *flex_shrink),
        _ => (None, None, 0.0, 1.0),
    };
    LayoutInputNode {
        id,
        parent_id,
        op,
        children_ids,
        debug_name: format!("node-{}", id.as_u128()),
        width,
        height,
        flex_grow,
        flex_shrink,
        rich_text: None,
    }
}

fn text_run(text: &str) -> TextRun {
    TextRun {
        text: text.to_owned(),
        style: TextStyle {
            font_size: 16.0,
            color: Color::BLACK,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
            typography: Default::default(),
        },
    }
}

impl RecordingMeasurer {
    fn new() -> Self {
        Self {
            last_font_size_bits: AtomicU32::new(f32::NAN.to_bits()),
        }
    }

    fn last_font_size(&self) -> f32 {
        f32::from_bits(self.last_font_size_bits.load(Ordering::SeqCst))
    }
}

impl TextMeasurer for RecordingMeasurer {
    fn measure(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn hit_test(
        &self,
        _text: &str,
        font_size: f32,
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
    ) -> usize {
        self.last_font_size_bits
            .store(font_size.to_bits(), Ordering::SeqCst);
        0
    }
}

#[test]
fn matches_input_nodes_rejects_reordered_flattened_inputs() {
    let root = WidgetId::from_u128(1);
    let first = WidgetId::from_u128(2);
    let second = WidgetId::from_u128(3);
    let canonical = vec![
        box_node(root, None, vec![first, second]),
        box_node(first, Some(root), vec![]),
        box_node(second, Some(root), vec![]),
    ];
    let reordered = vec![
        box_node(root, None, vec![first, second]),
        box_node(second, Some(root), vec![]),
        box_node(first, Some(root), vec![]),
    ];

    let state = LayoutGraphState::from_input_nodes(&canonical, 1);
    assert!(!state.matches_input_nodes(&reordered));
}

#[test]
fn update_refreshes_node_order_for_reordered_flattened_inputs() {
    let root = WidgetId::from_u128(10);
    let first = WidgetId::from_u128(11);
    let second = WidgetId::from_u128(12);
    let canonical = vec![
        box_node(root, None, vec![first, second]),
        box_node(first, Some(root), vec![]),
        box_node(second, Some(root), vec![]),
    ];
    let reordered = vec![
        box_node(root, None, vec![first, second]),
        box_node(second, Some(root), vec![]),
        box_node(first, Some(root), vec![]),
    ];

    let mut engine = LayoutEngine::new();
    engine.update(&canonical);
    engine.update(&reordered);

    let ordered = engine
        .graph_state
        .ordered_nodes()
        .map(|node| node.id)
        .collect::<Vec<_>>();
    assert_eq!(ordered, vec![root, second, first]);
}

#[test]
fn rich_text_hit_test_uses_body_font_size_when_runs_are_empty() {
    let measurer = RecordingMeasurer::new();

    measurer.hit_test_rich(&[], None, 4.0, 2.0);

    assert_eq!(
        measurer.last_font_size(),
        DEFAULT_RICH_TEXT_HIT_TEST_FONT_SIZE
    );
}

#[test]
fn rich_text_hit_test_uses_first_run_font_size_when_present() {
    let measurer = RecordingMeasurer::new();
    let runs = vec![TextRun {
        text: "Hello".to_string(),
        style: TextStyle {
            font_size: 18.0,
            color: Color::BLACK,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
            typography: Default::default(),
        },
    }];

    measurer.hit_test_rich(&runs, None, 4.0, 2.0);

    assert_eq!(measurer.last_font_size(), 18.0);
}

#[test]
fn typed_lengths_resolve_calc_clamp_and_viewport_units() {
    let viewport = LayoutSize::new(1200.0, 800.0);
    let calculated = Length::percent(50.0) - Length::points(24.0);
    let clamped = Length::clamp(Length::points(100.0), calculated, Length::vw(40.0));

    assert_eq!(resolve_length(&clamped, 600.0, viewport), Some(276.0));
    assert_eq!(
        resolve_length(&Length::vh(25.0), 0.0, viewport),
        Some(200.0)
    );
    assert_eq!(
        Length::points(10.0).resolve(0.0, viewport.width, viewport.height),
        Some(10.0)
    );
    assert_eq!(
        (Length::points(10.0) - Length::points(24.0)).resolve(0.0, viewport.width, viewport.height),
        Some(-14.0),
        "signed expressions remain available to typed positioning"
    );
    assert_eq!(
        resolve_position_length(&Length::points(-128.0), 600.0, viewport),
        Some(-128.0),
        "position offsets preserve signed values"
    );
    assert_eq!(
        Length::min(vec![Length::points(10.0), Length::MaxContent]).resolve(
            100.0,
            viewport.width,
            viewport.height
        ),
        None,
        "intrinsic expressions must be measured rather than partially resolved"
    );
}

#[test]
fn responsive_container_query_selects_from_parent_constraints() {
    let root = WidgetId::from_u128(100);
    let responsive = WidgetId::from_u128(101);
    let compact = WidgetId::from_u128(102);
    let wide = WidgetId::from_u128(103);
    let nodes = vec![
        node(
            root,
            None,
            vec![responsive],
            LayoutOp::Box {
                width: Some(240.0),
                height: Some(100.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            },
        ),
        node(
            responsive,
            Some(root),
            vec![compact, wide],
            LayoutOp::Responsive {
                query: ResponsiveQuery::Container,
                cases: vec![ResponsiveCondition {
                    min_width: None,
                    max_width: Some(300.0),
                }],
            },
        ),
        box_node(compact, Some(responsive), vec![]),
        box_node(wide, Some(responsive), vec![]),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .expect("responsive layout");

    assert!(snapshot.nodes.contains_key(&compact));
    assert!(!snapshot.nodes.contains_key(&wide));
}

#[test]
fn responsive_cases_use_first_match_precedence() {
    let root = WidgetId::from_u128(110);
    let responsive = WidgetId::from_u128(111);
    let first_match = WidgetId::from_u128(112);
    let later_match = WidgetId::from_u128(113);
    let fallback = WidgetId::from_u128(114);
    let nodes = vec![
        node(
            root,
            None,
            vec![responsive],
            LayoutOp::Box {
                width: Some(500.0),
                height: Some(100.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            },
        ),
        node(
            responsive,
            Some(root),
            vec![first_match, later_match, fallback],
            LayoutOp::Responsive {
                query: ResponsiveQuery::Viewport,
                cases: vec![
                    ResponsiveCondition {
                        min_width: None,
                        max_width: Some(900.0),
                    },
                    ResponsiveCondition {
                        min_width: None,
                        max_width: Some(600.0),
                    },
                ],
            },
        ),
        box_node(first_match, Some(responsive), vec![]),
        box_node(later_match, Some(responsive), vec![]),
        box_node(fallback, Some(responsive), vec![]),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(500.0, 600.0), &|_| 0.0)
        .expect("responsive layout");

    assert!(snapshot.nodes.contains_key(&first_match));
    assert!(!snapshot.nodes.contains_key(&later_match));
    assert!(!snapshot.nodes.contains_key(&fallback));
}

#[test]
fn grid_repeat_and_spans_are_applied_by_the_layout_engine() {
    let root = WidgetId::from_u128(200);
    let first = WidgetId::from_u128(201);
    let second = WidgetId::from_u128(202);
    let nodes = vec![
        node(
            root,
            None,
            vec![first, second],
            LayoutOp::Grid {
                columns: vec![GridTrack::repeat(2, vec![GridTrack::Points(50.0)])],
                rows: vec![GridTrack::Points(20.0)],
                column_gap: Some(10.0),
                row_gap: None,
                padding: [0.0; 4],
            },
        ),
        box_node(first, Some(root), vec![]),
        box_node(second, Some(root), vec![]),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(110.0, 20.0), &|_| 0.0)
        .expect("grid layout");

    assert_eq!(snapshot.nodes[&first].rect.x(), 0.0);
    assert_eq!(snapshot.nodes[&second].rect.x(), 60.0);
}

#[test]
fn auto_grid_items_advance_past_occupied_spans() {
    let root = WidgetId::from_u128(250);
    let first = WidgetId::from_u128(251);
    let first_child = WidgetId::from_u128(252);
    let second = WidgetId::from_u128(253);
    let second_child = WidgetId::from_u128(254);
    let nodes = vec![
        node(
            root,
            None,
            vec![first, second],
            LayoutOp::Grid {
                columns: vec![GridTrack::Points(50.0), GridTrack::Points(50.0)],
                rows: vec![],
                column_gap: None,
                row_gap: None,
                padding: [0.0; 4],
            },
        ),
        node(
            first,
            Some(root),
            vec![first_child],
            LayoutOp::GridItem {
                row_start: GridPlacement::Auto,
                row_end: GridPlacement::Auto,
                col_start: GridPlacement::Auto,
                col_end: GridPlacement::Span(2),
            },
        ),
        box_node(first_child, Some(first), vec![]),
        node(
            second,
            Some(root),
            vec![second_child],
            LayoutOp::GridItem {
                row_start: GridPlacement::Auto,
                row_end: GridPlacement::Auto,
                col_start: GridPlacement::Auto,
                col_end: GridPlacement::Auto,
            },
        ),
        box_node(second_child, Some(second), vec![]),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(100.0, 100.0), &|_| 0.0)
        .expect("auto grid layout");

    assert_eq!(snapshot.nodes[&first].rect.x(), 0.0);
    assert_eq!(snapshot.nodes[&first].rect.width(), 100.0);
    assert_eq!(snapshot.nodes[&second].rect.x(), 0.0);
    assert_eq!(snapshot.nodes[&second].rect.y(), 20.0);
}

#[test]
fn fixed_text_box_retains_natural_size_for_overflow_inspection() {
    let root = WidgetId::from_u128(300);
    let mut text = node(
        root,
        None,
        vec![],
        LayoutOp::StyledBox {
            style: BoxStyle {
                width: Some(Length::Points(40.0)),
                height: Some(Length::Points(10.0)),
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        },
    );
    text.rich_text = Some(vec![text_run("overflowing text")]);
    let nodes = vec![text];
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(WrappingMeasurer));
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(100.0, 100.0), &|_| 0.0)
        .expect("text layout");
    let inspection = engine
        .inspect_node(&snapshot, root)
        .expect("layout inspection");

    assert_eq!(inspection.laid_out.width(), 40.0);
    assert_eq!(inspection.laid_out.height(), 10.0);
    assert!(inspection.measured.height() > inspection.laid_out.height());
    assert!(inspection.overflow_y);
    assert_eq!(
        inspection.constrained, inspection.laid_out,
        "fixed constraints should match final bounds"
    );
}

#[test]
fn max_content_box_propagates_unwrapped_text_width() {
    let root = WidgetId::from_u128(400);
    let text_id = WidgetId::from_u128(401);
    let mut text = node(
        text_id,
        Some(root),
        vec![],
        LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
    );
    text.rich_text = Some(vec![text_run("hello world")]);
    let nodes = vec![
        node(
            root,
            None,
            vec![text_id],
            LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::MaxContent),
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 1.0,
            },
        ),
        text,
    ];
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(WrappingMeasurer));
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(300.0, 100.0), &|_| 0.0)
        .expect("max-content layout");

    assert_eq!(snapshot.nodes[&root].rect.width(), 110.0);
    assert_eq!(snapshot.nodes[&text_id].rect.width(), 110.0);
}

#[test]
fn intrinsic_lengths_participate_in_clamp_expressions() {
    let root = WidgetId::from_u128(450);
    let mut text = node(
        root,
        None,
        vec![],
        LayoutOp::StyledBox {
            style: BoxStyle {
                width: Some(Length::clamp(
                    Length::points(50.0),
                    Length::MaxContent,
                    Length::points(80.0),
                )),
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        },
    );
    text.rich_text = Some(vec![text_run("hello world")]);
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(WrappingMeasurer));
    let snapshot = engine
        .compute_layout(&[text], root, LayoutSize::new(300.0, 100.0), &|_| 0.0)
        .expect("intrinsic clamp layout");

    assert_eq!(snapshot.nodes[&root].rect.width(), 80.0);
    assert_eq!(snapshot.nodes[&root].rect.height(), 40.0);
}

#[test]
fn margin_wrapper_keeps_percentage_width_relative_to_the_containing_box() {
    let outer = WidgetId::from_u128(455);
    let inner = WidgetId::from_u128(456);
    let nodes = vec![
        node(
            outer,
            None,
            vec![inner],
            LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(
                        Length::percent(50.0) + Length::points(12.0) + Length::points(12.0),
                    ),
                    padding: Some(Length::all(Length::points(12.0))),
                    alignment: fission_ir::op::BoxAlignment::Stretch,
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 1.0,
            },
        ),
        node(
            inner,
            Some(outer),
            vec![],
            LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::percent(100.0)),
                    height: Some(Length::points(20.0)),
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 1.0,
            },
        ),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, outer, LayoutSize::new(200.0, 100.0), &|_| 0.0)
        .expect("margin layout");

    assert_eq!(snapshot.nodes[&outer].rect.width(), 124.0);
    assert_eq!(snapshot.nodes[&inner].rect.width(), 100.0);
    assert_eq!(snapshot.nodes[&inner].rect.x(), 12.0);
}

#[test]
fn fit_content_height_preserves_wrapped_text_height() {
    let root = WidgetId::from_u128(460);
    let mut text = node(
        root,
        None,
        vec![],
        LayoutOp::StyledBox {
            style: BoxStyle {
                width: Some(Length::points(40.0)),
                height: Some(Length::fit_content(None)),
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        },
    );
    text.rich_text = Some(vec![text_run("abcdefgh")]);
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(WrappingMeasurer));
    let snapshot = engine
        .compute_layout(&[text], root, LayoutSize::new(300.0, 100.0), &|_| 0.0)
        .expect("fit-content height layout");

    assert_eq!(snapshot.nodes[&root].rect.width(), 40.0);
    assert_eq!(snapshot.nodes[&root].rect.height(), 40.0);
}

#[test]
fn typed_position_offsets_resolve_against_the_parent_box() {
    let root = WidgetId::from_u128(500);
    let positioned = WidgetId::from_u128(501);
    let child = WidgetId::from_u128(502);
    let nodes = vec![
        node(root, None, vec![positioned], LayoutOp::ZStack),
        node(
            positioned,
            Some(root),
            vec![child],
            LayoutOp::PositionedLengths {
                left: Some(Length::Percent(25.0)),
                top: Some(Length::Percent(10.0)),
                right: None,
                bottom: None,
                width: Some(Length::Points(50.0)),
                height: Some(Length::Points(20.0)),
            },
        ),
        node(
            child,
            Some(positioned),
            vec![],
            LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            },
        ),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(200.0, 100.0), &|_| 0.0)
        .expect("typed positioned layout");

    assert_eq!(snapshot.nodes[&child].rect.x(), 50.0);
    assert_eq!(snapshot.nodes[&child].rect.y(), 10.0);
    assert_eq!(snapshot.nodes[&child].rect.width(), 50.0);
    assert_eq!(snapshot.nodes[&child].rect.height(), 20.0);
}

#[test]
fn anchored_position_uses_the_childs_intrinsic_size() {
    let root = WidgetId::from_u128(510);
    let positioned = WidgetId::from_u128(511);
    let child = WidgetId::from_u128(512);
    let nodes = vec![
        node(root, None, vec![positioned], LayoutOp::ZStack),
        node(
            positioned,
            Some(root),
            vec![child],
            LayoutOp::AnchoredPositioned {
                x: 100.0,
                y: 50.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
            },
        ),
        node(
            child,
            Some(positioned),
            vec![],
            LayoutOp::Box {
                width: Some(40.0),
                height: Some(20.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            },
        ),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(200.0, 100.0), &|_| 0.0)
        .expect("anchored positioned layout");

    assert_eq!(
        snapshot.nodes[&child].rect,
        LayoutRect::new(80.0, 40.0, 40.0, 20.0)
    );
}

#[test]
fn a_centred_styled_box_places_a_smaller_child_in_the_middle() {
    // A fixed 36px cell centring a 20px line of text, like a pagination ellipsis. The
    // child keeps its own height and sits 8px down; given the cell's height it would
    // fill the cell and its text would sit at the top.
    let root = WidgetId::from_u128(910);
    let cell = WidgetId::from_u128(911);
    let child = WidgetId::from_u128(912);
    let nodes = vec![
        node(
            root,
            None,
            vec![cell],
            LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Row,
                wrap: fission_ir::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                line_gap: None,
                align_items: fission_ir::op::AlignItems::Start,
                justify_content: fission_ir::op::JustifyContent::Start,
            },
        ),
        node(
            cell,
            Some(root),
            vec![child],
            LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::Points(36.0)),
                    height: Some(Length::Points(36.0)),
                    alignment: fission_ir::op::BoxAlignment::Center,
                    ..BoxStyle::default()
                },
                flex_grow: 0.0,
                flex_shrink: 0.0,
            },
        ),
        node(
            child,
            Some(cell),
            vec![],
            LayoutOp::Box {
                width: Some(12.0),
                height: Some(20.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            },
        ),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(400.0, 200.0), &|_| 0.0)
        .expect("centred cell layout");

    assert_eq!(
        snapshot.nodes[&child].rect,
        LayoutRect::new(12.0, 8.0, 12.0, 20.0),
        "the child keeps its height and is centred in the cell"
    );
}

#[test]
fn a_box_in_a_flex_line_centres_content_within_its_own_minimum_height() {
    // A button-shaped box: 48px minimum height, horizontal padding, and a column that
    // centres a 20px label. In a row that does not stretch it, the box keeps its own
    // height and the label sits in the middle rather than at the top.
    let row = WidgetId::from_u128(900);
    let control = WidgetId::from_u128(901);
    let column = WidgetId::from_u128(902);
    let label = WidgetId::from_u128(903);
    let nodes = vec![
        node(
            row,
            None,
            vec![control],
            LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Row,
                wrap: fission_ir::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                line_gap: None,
                align_items: fission_ir::op::AlignItems::Start,
                justify_content: fission_ir::op::JustifyContent::Start,
            },
        ),
        node(
            control,
            Some(row),
            vec![column],
            LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: Some(48.0),
                max_height: None,
                padding: [16.0, 16.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            },
        ),
        node(
            column,
            Some(control),
            vec![label],
            LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Column,
                wrap: fission_ir::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                line_gap: None,
                align_items: fission_ir::op::AlignItems::Center,
                justify_content: fission_ir::op::JustifyContent::Center,
            },
        ),
        node(
            label,
            Some(column),
            vec![],
            LayoutOp::Box {
                width: Some(60.0),
                height: Some(20.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            },
        ),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, row, LayoutSize::new(400.0, 200.0), &|_| 0.0)
        .expect("button-shaped layout");

    assert_eq!(
        snapshot.nodes[&control].rect.size.height, 48.0,
        "the control keeps its own height instead of filling the row"
    );
    assert_eq!(
        snapshot.nodes[&label].rect,
        LayoutRect::new(16.0, 14.0, 60.0, 20.0),
        "the label is centred inside the control's minimum height"
    );
}

#[test]
fn spotlight_lays_out_inverse_overlay_around_anchor() {
    let root = WidgetId::from_u128(20);
    let positioned = WidgetId::from_u128(21);
    let anchor = WidgetId::from_u128(22);
    let spotlight = WidgetId::from_u128(23);
    let panels = (24..=28).map(WidgetId::from_u128).collect::<Vec<_>>();

    let mut nodes = vec![
        LayoutInputNode {
            id: root,
            parent_id: None,
            op: LayoutOp::ZStack,
            children_ids: vec![positioned, spotlight],
            debug_name: "root".into(),
            width: None,
            height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            rich_text: None,
        },
        LayoutInputNode {
            id: positioned,
            parent_id: Some(root),
            op: LayoutOp::Positioned {
                left: Some(100.0),
                top: Some(100.0),
                right: None,
                bottom: None,
                width: Some(200.0),
                height: Some(80.0),
            },
            children_ids: vec![anchor],
            debug_name: "positioned-anchor".into(),
            width: Some(200.0),
            height: Some(80.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            rich_text: None,
        },
        box_node(anchor, Some(positioned), vec![]),
        LayoutInputNode {
            id: spotlight,
            parent_id: Some(root),
            op: LayoutOp::Spotlight {
                anchor,
                padding: 12.0,
            },
            children_ids: panels.clone(),
            debug_name: "spotlight".into(),
            width: None,
            height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            rich_text: None,
        },
    ];
    nodes[2].op = LayoutOp::Box {
        width: Some(200.0),
        height: Some(80.0),
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
        padding: [0.0; 4],
        flex_grow: 0.0,
        flex_shrink: 0.0,
        aspect_ratio: None,
    };
    nodes[2].width = Some(200.0);
    nodes[2].height = Some(80.0);
    nodes.extend(
        panels
            .iter()
            .map(|id| box_node(*id, Some(spotlight), vec![])),
    );

    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .expect("spotlight layout");

    let expected = [
        LayoutRect::new(0.0, 0.0, 800.0, 88.0),
        LayoutRect::new(0.0, 192.0, 800.0, 408.0),
        LayoutRect::new(0.0, 88.0, 88.0, 104.0),
        LayoutRect::new(312.0, 88.0, 488.0, 104.0),
        LayoutRect::new(88.0, 88.0, 224.0, 104.0),
    ];
    for (panel, expected_rect) in panels.iter().zip(expected) {
        assert_eq!(snapshot.get_node_rect(*panel), Some(expected_rect));
    }
}

#[test]
fn flyout_placement_clamps_rendered_descendants_inside_viewport() {
    let position = flyout_root_position(
        LayoutSize::new(800.0, 600.0),
        LayoutRect::new(700.0, 550.0, 80.0, 32.0),
        LayoutRect::new(0.0, 8.0, 440.0, 220.0),
        Default::default(),
    );

    assert_eq!(position, LayoutPoint::new(360.0, 322.0));
}

#[test]
fn flyout_placement_prefers_below_when_full_content_fits() {
    let position = flyout_root_position(
        LayoutSize::new(800.0, 600.0),
        LayoutRect::new(100.0, 100.0, 200.0, 80.0),
        LayoutRect::new(0.0, 8.0, 320.0, 180.0),
        Default::default(),
    );

    assert_eq!(position, LayoutPoint::new(100.0, 172.0));
}

fn fixed_height_child(id: WidgetId, parent: WidgetId, height: f32) -> LayoutInputNode {
    node(
        id,
        Some(parent),
        vec![],
        LayoutOp::Box {
            width: Some(80.0),
            height: Some(height),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
    )
}

/// A column taller than its container spills past the bottom edge, as it does on
/// the web, instead of squeezing every child until the text inside overlaps.
#[test]
fn an_overflowing_column_keeps_each_child_at_its_content_height() {
    let root = WidgetId::from_u128(900);
    let first = WidgetId::from_u128(901);
    let second = WidgetId::from_u128(902);
    let nodes = vec![
        node(
            root,
            None,
            vec![first, second],
            LayoutOp::Flex {
                direction: fission_ir::op::FlexDirection::Column,
                wrap: fission_ir::op::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                line_gap: None,
                align_items: fission_ir::op::AlignItems::Start,
                justify_content: fission_ir::op::JustifyContent::Start,
            },
        ),
        fixed_height_child(first, root, 60.0),
        fixed_height_child(second, root, 60.0),
    ];

    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(200.0, 100.0), &|_| 0.0)
        .expect("column layout");

    let first_rect = snapshot.nodes[&first].rect;
    let second_rect = snapshot.nodes[&second].rect;
    assert_eq!(
        first_rect.height(),
        60.0,
        "the first child keeps its height"
    );
    assert_eq!(
        second_rect.height(),
        60.0,
        "the second child keeps its height"
    );
    assert!(
        second_rect.y() >= first_rect.y() + first_rect.height(),
        "the second child starts below the first, at {} against {}",
        second_rect.y(),
        first_rect.y() + first_rect.height()
    );
}
