use std::sync::Arc;

use fission_core::authoring::LoweringContext;
use fission_core::env::{Env, RuntimeState};
use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
use fission_core::ui::{RichText, RichTextRun};
use fission_core::{NavigationCommand, Runtime};
use fission_ir::{CoreIR, CoreNode, LayoutOp, Op, Role, Semantics, WidgetId};
use fission_layout::{
    LayoutEngine, LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LineMetric,
    ParagraphSelectionBox, ResolvedParagraphLayout, TextMeasurer,
};

const LINK_URL: &str = "https://example.com/guide";

#[derive(Debug)]
struct LinkTextMeasurer;

impl TextMeasurer for LinkTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (text.chars().count() as f32 * 10.0, 20.0)
    }

    fn resolve_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        available_width: Option<f32>,
    ) -> ResolvedParagraphLayout {
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        let selection_boxes = text
            .char_indices()
            .map(|(start, character)| ParagraphSelectionBox {
                start_index: start,
                end_index: start + character.len_utf8(),
                line_index: 0,
                rect: LayoutRect::new(start as f32 * 10.0, 0.0, 10.0, 20.0),
            })
            .collect::<Vec<_>>();

        ResolvedParagraphLayout {
            constraint_width: available_width,
            size: LayoutSize::new(text.chars().count() as f32 * 10.0, 20.0),
            lines: vec![LineMetric {
                start_index: 0,
                end_index: text.len(),
                baseline: 15.0,
                height: 20.0,
                width: text.chars().count() as f32 * 10.0,
            }],
            inline_boxes: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            caret_stops: Vec::new(),
            selection_boxes,
        }
    }

    fn resolve_rich_text_annotation_at_point(
        &self,
        _runs: &[fission_ir::op::TextRun],
        _available_width: Option<f32>,
        x: f32,
        y: f32,
        _paragraph_style: fission_ir::op::TextParagraphStyle,
        annotations: &[fission_ir::op::RichTextAnnotation],
    ) -> Option<fission_ir::op::RichTextAnnotation> {
        (50.0..100.0)
            .contains(&x)
            .then(|| {
                annotations
                    .iter()
                    .find(|annotation| annotation.hyperlink.is_some())
            })
            .flatten()
            .filter(|_| (0.0..20.0).contains(&y))
            .cloned()
    }
}

fn linked_text_frame() -> (
    CoreIR,
    fission_layout::LayoutSnapshot,
    Arc<dyn TextMeasurer>,
) {
    let measurer: Arc<dyn TextMeasurer> = Arc::new(LinkTextMeasurer);
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, Some(&measurer), None);
    let root = fission_core::internal::lower_widget(
        &RichText::new(vec![
            RichTextRun::new("Read "),
            RichTextRun::new("guide")
                .semantics_identifier("rich-text-guide")
                .hyperlink(fission_ir::Hyperlink::new(LINK_URL)),
        ])
        .width(200.0)
        .into(),
        &mut cx,
    );
    cx.set_root(root);
    let input_nodes = fission_core::internal::build_layout_tree(cx.ir(), &env);
    let mut engine = LayoutEngine::new().with_measurer(measurer.clone());
    engine
        .rebuild(&input_nodes)
        .expect("build text layout input");
    let layout = engine
        .compute_layout(&input_nodes, root, LayoutSize::new(220.0, 40.0), &|_| 0.0)
        .expect("layout linked rich text");
    (cx.into_ir(), layout, measurer)
}

fn link_semantics_id(ir: &CoreIR) -> WidgetId {
    ir.nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(semantics)
                if semantics.role == Role::Link
                    && semantics.identifier.as_deref() == Some("rich-text-guide") =>
            {
                Some(*id)
            }
            _ => None,
        })
        .expect("lowered link semantics")
}

fn linked_text_overlay_frame_with_text_input() -> (
    CoreIR,
    fission_layout::LayoutSnapshot,
    Arc<dyn TextMeasurer>,
    WidgetId,
    WidgetId,
) {
    let (mut ir, mut layout, measurer) = linked_text_frame();
    let root = ir.root.expect("rich text root");
    let input = WidgetId::explicit("underlying-text-input");
    let overlay_children = ir
        .nodes
        .get(&root)
        .expect("rich text root node")
        .children
        .clone();
    {
        let root_node = ir.nodes.get_mut(&root).expect("rich text root node");
        root_node.op = Op::Layout(LayoutOp::ZStack);
        root_node.children = std::iter::once(input)
            .chain(overlay_children.iter().copied())
            .collect();
    }
    for child in &overlay_children {
        ir.nodes.get_mut(child).expect("root child").parent = Some(root);
    }
    ir.nodes.insert(
        input,
        CoreNode {
            id: input,
            parent: Some(root),
            children: Vec::new(),
            op: Op::Semantics(Semantics {
                role: Role::TextInput,
                value: Some("Read guide".into()),
                focusable: true,
                focus_policy: fission_ir::FocusPolicy::FocusOnPointer,
                text_editable: true,
                multiline: true,
                ..Semantics::default()
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );
    let root_rect = LayoutRect::new(0.0, 0.0, 220.0, 40.0);
    layout.nodes.insert(
        root,
        LayoutNodeGeometry {
            rect: root_rect,
            content_size: LayoutSize::new(220.0, 40.0),
        },
    );
    layout.nodes.insert(
        input,
        LayoutNodeGeometry {
            rect: root_rect,
            content_size: root_rect.size,
        },
    );
    let paint = ir
        .nodes
        .iter()
        .find_map(|(id, node)| {
            matches!(node.op, Op::Paint(fission_ir::PaintOp::DrawRichText { .. })).then_some(*id)
        })
        .expect("rich text paint node");
    (ir, layout, measurer, input, paint)
}

#[test]
fn rich_text_overlay_passes_non_link_and_blank_taps_to_underlying_text_input() {
    let (ir, layout, measurer, input, paint) = linked_text_overlay_frame_with_text_input();
    let runtime = Runtime::default();
    let hit = |x| {
        fission_core::hit_test::hit_test_with_scroll(
            &ir,
            &layout,
            &runtime.runtime_state.scroll,
            LayoutPoint::new(x, 10.0),
        )
    };

    assert_eq!(hit(20.0), Some(input), "ordinary text should pass through");
    assert_eq!(
        hit(180.0),
        Some(input),
        "blank overlay space should pass through"
    );
    assert_eq!(
        hit(75.0),
        Some(paint),
        "the linked range remains interactive"
    );

    let tap = |runtime: &mut Runtime, point: LayoutPoint| {
        runtime
            .handle_input(
                InputEvent::Pointer(PointerEvent::Down {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                &ir,
                &layout,
            )
            .expect("pointer down");
        runtime
            .handle_input(
                InputEvent::Pointer(PointerEvent::Up {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                &ir,
                &layout,
            )
            .expect("pointer up");
    };

    for (x, label) in [(20.0, "ordinary text"), (180.0, "blank overlay space")] {
        let mut runtime = Runtime::default().with_measurer(measurer.clone());
        tap(&mut runtime, LayoutPoint::new(x, 10.0));
        assert_eq!(
            runtime.runtime_state.interaction.focused,
            Some(input),
            "a tap on {label} should focus the underlying TextInput"
        );
    }

    let mut runtime = Runtime::default().with_measurer(measurer);
    tap(&mut runtime, LayoutPoint::new(75.0, 10.0));
    assert_eq!(
        runtime.take_pending_navigation(),
        vec![NavigationCommand::Open(fission_ir::Hyperlink::new(
            LINK_URL
        ))],
        "a tap on the annotated hyperlink should still navigate"
    );
}

#[test]
fn pointer_activation_opens_link_from_rich_text_annotation() {
    let (ir, layout, measurer) = linked_text_frame();
    let mut runtime = Runtime::default().with_measurer(measurer);
    let point = LayoutPoint::new(75.0, 10.0);

    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Down {
                pointer_id: Default::default(),
                kind: Default::default(),
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .expect("pointer down");
    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Up {
                pointer_id: Default::default(),
                kind: Default::default(),
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .expect("pointer up");

    assert_eq!(
        runtime.take_pending_navigation(),
        vec![NavigationCommand::Open(fission_ir::Hyperlink::new(
            LINK_URL
        ))]
    );
}

#[test]
fn keyboard_focus_and_enter_activate_rich_text_link() {
    let (ir, layout, measurer) = linked_text_frame();
    let link = link_semantics_id(&ir);
    assert!(
        fission_core::ui::widgets::text::rich_text_link_rects(&ir, &layout, link).is_some(),
        "the retained link must resolve its visual range in the shaped paragraph"
    );
    let mut runtime = Runtime::default().with_measurer(measurer);

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Tab,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .expect("tab to the link");
    assert_eq!(runtime.runtime_state.interaction.focused, Some(link));

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Enter,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .expect("activate focused link");
    assert_eq!(
        runtime.take_pending_navigation(),
        vec![NavigationCommand::Open(fission_ir::Hyperlink::new(
            LINK_URL
        ))]
    );
}
