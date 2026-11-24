use super::*;

#[test]
fn get_tree_metadata_keeps_identifier_separate_and_masks_value() {
    let input = WidgetId::from_u128(10);
    let mut ir = CoreIR::new();
    ir.add_node(
        input,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            label: Some("Password".into()),
            identifier: Some("account.password".into()),
            value: Some("hunter2".into()),
            focusable: true,
            masked: true,
            text_selection: Some((1, 3)),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.set_root(input);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
    snapshot.nodes.insert(
        input,
        LayoutNodeGeometry {
            rect: LayoutRect::new(20.0, 30.0, 160.0, 32.0),
            content_size: LayoutSize::new(160.0, 32.0),
        },
    );

    let records = collect_semantic_records(&ir, &snapshot, &ScrollStateMap::default());
    assert_eq!(records.len(), 1);
    let node = &records[0].node;
    assert_eq!(node.identifier.as_deref(), Some("account.password"));
    assert_eq!(node.label.as_deref(), Some("Password"));
    assert_eq!(node.value, None);
    assert!(node.value_present);
    assert!(node.masked);
    assert_eq!(node.text_selection, Some((1, 3)));
    assert_eq!(
        node.visibility,
        fission_test_driver::VisibilityState::FullyVisible
    );
}

#[test]
fn get_tree_excludes_semantics_from_inactive_responsive_branches() {
    let root = WidgetId::explicit("responsive-root");
    let compact_branch = WidgetId::explicit("compact-branch");
    let compact_input = WidgetId::explicit("compact-input");
    let expanded_branch = WidgetId::explicit("expanded-branch");
    let expanded_input = WidgetId::explicit("expanded-input");
    let input_semantics = || {
        Op::Semantics(Semantics {
            role: Role::TextInput,
            identifier: Some("slide.text.input".into()),
            focusable: true,
            value: Some("Draft".into()),
            ..Semantics::default()
        })
    };
    let box_op = || {
        Op::Layout(LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        })
    };

    let mut ir = CoreIR::new();
    ir.add_node(compact_input, input_semantics(), Vec::new());
    ir.add_node(compact_branch, box_op(), vec![compact_input]);
    ir.add_node(expanded_input, input_semantics(), Vec::new());
    ir.add_node(expanded_branch, box_op(), vec![expanded_input]);
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Responsive {
            query: fission_ir::op::ResponsiveQuery::Viewport,
            cases: vec![fission_ir::op::ResponsiveCondition {
                min_width: None,
                max_width: Some(600.0),
            }],
        }),
        vec![compact_branch, expanded_branch],
    );
    ir.set_root(root);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(1_100.0, 700.0));
    for (id, rect) in [
        (root, LayoutRect::new(0.0, 0.0, 1_100.0, 700.0)),
        (expanded_branch, LayoutRect::new(0.0, 0.0, 1_100.0, 700.0)),
        (expanded_input, LayoutRect::new(20.0, 20.0, 320.0, 36.0)),
    ] {
        snapshot.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect,
                content_size: LayoutSize::new(rect.width(), rect.height()),
            },
        );
    }

    let records = collect_semantic_records(&ir, &snapshot, &ScrollStateMap::default());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, expanded_input);
    assert_eq!(
        records[0].node.identifier.as_deref(),
        Some("slide.text.input")
    );
}

#[test]
fn web_semantic_records_project_rich_text_links_to_line_fragments() {
    let root = WidgetId::explicit("web-rich-link-root");
    let paint = WidgetId::explicit("web-rich-link-paint");
    let link = WidgetId::explicit("web-rich-link-semantics");
    let hyperlink = fission_ir::Hyperlink::new("https://example.com/guide");
    let mut ir = CoreIR::new();
    ir.add_node(
        link,
        Op::Semantics(Semantics {
            role: Role::Link,
            label: Some("docs".into()),
            identifier: Some("docs-link".into()),
            hyperlink: Some(hyperlink.clone()),
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        paint,
        Op::Paint(fission_ir::PaintOp::DrawRichText {
            runs: vec![fission_ir::op::TextRun {
                text: "Read docs".into(),
                style: fission_ir::op::TextStyle {
                    font_size: 16.0,
                    color: fission_ir::op::Color::BLACK,
                    underline: false,
                    font_family: None,
                    locale: None,
                    font_weight: 400,
                    font_style: fission_ir::op::FontStyle::Normal,
                    line_height: None,
                    letter_spacing: 0.0,
                    background_color: None,
                    typography: Default::default(),
                },
            }],
            display_breaks: Vec::new(),
            paragraph_styles: Vec::new(),
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        vec![link],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![paint],
    );
    ir.set_root(root);
    ir.custom_render_objects.insert(
        paint,
        std::sync::Arc::new(vec![fission_ir::op::RichTextAnnotation {
            range: 5..9,
            semantics_label: Some("docs".into()),
            semantics_identifier: Some("docs-link".into()),
            spell_out: None,
            mouse_cursor: Some(fission_ir::op::MouseCursor::Pointer),
            actions: Vec::new(),
            hyperlink: Some(hyperlink),
        }]),
    );

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(160.0, 40.0));
    for (id, rect, content_size) in [
        (
            root,
            LayoutRect::new(0.0, 0.0, 160.0, 40.0),
            LayoutSize::new(160.0, 40.0),
        ),
        (
            paint,
            LayoutRect::new(10.0, 8.0, 100.0, 20.0),
            LayoutSize::new(100.0, 20.0),
        ),
    ] {
        snapshot
            .nodes
            .insert(id, LayoutNodeGeometry { rect, content_size });
    }
    snapshot.paragraphs.insert(
        paint,
        fission_layout::ResolvedParagraphLayout {
            constraint_width: Some(100.0),
            size: LayoutSize::new(100.0, 20.0),
            lines: Vec::new(),
            inline_boxes: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            caret_stops: Vec::new(),
            selection_boxes: vec![fission_layout::ParagraphSelectionBox {
                start_index: 5,
                end_index: 9,
                line_index: 0,
                rect: LayoutRect::new(50.0, 0.0, 40.0, 16.0),
            }],
            display_breaks: Vec::new(),
        },
    );

    let records = collect_semantic_records(&ir, &snapshot, &ScrollStateMap::default());
    let record = records
        .iter()
        .find(|record| record.node.identifier.as_deref() == Some("docs-link"))
        .expect("inline link semantic record");
    assert_eq!(
        record.semantics.hyperlink.as_ref().unwrap().href,
        "https://example.com/guide"
    );
    assert_eq!(record.link_fragments.len(), 1);
    assert_eq!(record.link_fragments[0].x, 60.0);
    assert_eq!(record.link_fragments[0].y, 8.0);
    assert_eq!(record.link_fragments[0].width, 40.0);
    assert_eq!(record.link_fragments[0].height, 16.0);
}

#[test]
fn get_tree_reports_effective_sequential_focusability() {
    let input = WidgetId::from_u128(10);
    let mut ir = CoreIR::new();
    ir.add_node(
        input,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            focusable: false,
            sequential_focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.set_root(input);

    let records = collect_semantic_records(
        &ir,
        &LayoutSnapshot::new(LayoutSize::new(320.0, 240.0)),
        &ScrollStateMap::default(),
    );

    assert_eq!(records.len(), 1);
    assert!(!records[0].node.focusable);
    assert!(!records[0].node.sequential_focusable);
}

#[test]
fn live_test_fill_text_preserves_bound_payload_and_reports_runtime_edit() {
    let (pipeline, target) = live_text_pipeline(Some(LiveTextAction("smtp_host".into()).encode()));
    let mut runtime = live_text_runtime();
    let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

    let response = handle_fill_text_selector(&query, "greenmail", &mut runtime, &pipeline);

    assert!(matches!(response, fission_test_driver::TestResponse::Ok {}));
    let (field, change) = runtime
        .get_global_state::<LiveTextState>()
        .and_then(|state| state.edit.as_ref())
        .expect("live text edit");
    assert_eq!(field, "smtp_host");
    assert_eq!(change.node_id, target);
    assert_eq!(change.new_text, "greenmail");
    assert_eq!(change.new_caret, "greenmail".len());
    assert_eq!(change.new_anchor, "greenmail".len());
}

#[test]
fn live_test_fill_text_uses_text_capability_for_editable_combobox() {
    let (mut pipeline, target) =
        live_text_pipeline(Some(LiveTextAction("destination".into()).encode()));
    let ir = pipeline.prev_ir.as_mut().expect("live text IR");
    let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&target).unwrap().op else {
        unreachable!();
    };
    semantics.role = Role::ComboBox;
    semantics.text_editable = true;
    let mut runtime = live_text_runtime();
    let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

    let response = handle_fill_text_selector(&query, "London", &mut runtime, &pipeline);

    assert!(matches!(response, fission_test_driver::TestResponse::Ok {}));
    assert_eq!(
        focused_text_input_id(&runtime, pipeline.prev_ir.as_ref()),
        Some(target)
    );
    let records = collect_semantic_records(
        pipeline.prev_ir.as_ref().unwrap(),
        pipeline.last_snapshot.as_ref().unwrap(),
        &ScrollStateMap::default(),
    );
    assert_eq!(records[0].node.role, "ComboBox");
    assert!(records[0].node.text_editable);
    let (field, change) = runtime
        .get_global_state::<LiveTextState>()
        .and_then(|state| state.edit.as_ref())
        .expect("live combobox edit");
    assert_eq!(field, "destination");
    assert_eq!(change.node_id, target);
    assert_eq!(change.new_text, "London");
}

#[test]
fn live_test_fill_text_rejects_noneditable_combobox() {
    let (mut pipeline, target) =
        live_text_pipeline(Some(LiveTextAction("destination".into()).encode()));
    let ir = pipeline.prev_ir.as_mut().expect("live text IR");
    let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&target).unwrap().op else {
        unreachable!();
    };
    semantics.role = Role::ComboBox;
    semantics.text_editable = false;
    let mut runtime = live_text_runtime();
    runtime.runtime_state.interaction.set_focused(Some(target));
    let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

    let response = handle_fill_text_selector(&query, "must-not-dispatch", &mut runtime, &pipeline);

    assert!(matches!(
        response,
        fission_test_driver::TestResponse::SelectorError { .. }
    ));
    assert_eq!(
        focused_text_input_id(&runtime, pipeline.prev_ir.as_ref()),
        None
    );
    assert!(runtime
        .get_global_state::<LiveTextState>()
        .expect("live text state")
        .edit
        .is_none());
}

#[test]
fn live_test_fill_text_fails_when_text_input_has_no_input_action() {
    let (pipeline, target) = live_text_pipeline(None);
    let mut runtime = live_text_runtime();
    let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

    let response = handle_fill_text_selector(&query, "not-retained", &mut runtime, &pipeline);

    match response {
        fission_test_driver::TestResponse::SelectorError { failure } => {
            assert_eq!(
                failure.kind,
                fission_test_driver::SelectorFailureKind::UnsupportedAction
            );
            assert_eq!(failure.message, "text input edit could not be dispatched");
        }
        other => panic!("expected selector error, got {other:?}"),
    }
    assert!(runtime
        .get_global_state::<LiveTextState>()
        .expect("live text state")
        .edit
        .is_none());
    assert!(runtime.runtime_state.text_edit.get(target).is_none());
}

#[test]
fn live_test_fill_text_fails_and_restores_editor_state_on_dispatch_error() {
    let (pipeline, target) = live_text_pipeline(Some(b"not-json".to_vec()));
    let mut runtime = live_text_runtime();
    runtime
        .runtime_state
        .text_edit
        .sync_from_runtime(target, "before", None, None, false);
    runtime
        .runtime_state
        .text_edit
        .set_caret(target, 2, Some(1));
    let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

    let response = handle_fill_text_selector(&query, "private-value", &mut runtime, &pipeline);

    assert!(matches!(
        response,
        fission_test_driver::TestResponse::SelectorError { .. }
    ));
    assert!(runtime
        .get_global_state::<LiveTextState>()
        .expect("live text state")
        .edit
        .is_none());
    let editor = runtime
        .runtime_state
        .text_edit
        .get(target)
        .expect("restored editor state");
    assert_eq!(editor.buffer.to_string(), "before");
    assert_eq!((editor.caret, editor.anchor), (2, 1));
}

#[test]
fn selector_can_scope_duplicate_labels() {
    let shell = WidgetId::from_u128(20);
    let app = WidgetId::from_u128(21);
    let shell_open = WidgetId::from_u128(22);
    let app_open = WidgetId::from_u128(23);

    let mut ir = CoreIR::new();
    ir.add_node(
        shell_open,
        Op::Semantics(Semantics {
            role: Role::Button,
            label: Some("Open".into()),
            identifier: Some("shell.open".into()),
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        app_open,
        Op::Semantics(Semantics {
            role: Role::Button,
            label: Some("Open".into()),
            identifier: Some("app.open".into()),
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        app,
        Op::Semantics(Semantics {
            role: Role::Generic,
            identifier: Some("mounted.app".into()),
            ..Semantics::default()
        }),
        vec![app_open],
    );
    ir.add_node(
        shell,
        Op::Semantics(Semantics {
            role: Role::Generic,
            identifier: Some("shell".into()),
            ..Semantics::default()
        }),
        vec![shell_open, app],
    );
    ir.set_root(shell);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
    for (id, y) in [
        (shell, 0.0),
        (app, 80.0),
        (shell_open, 8.0),
        (app_open, 88.0),
    ] {
        snapshot.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, y, 120.0, 40.0),
                content_size: LayoutSize::new(120.0, 40.0),
            },
        );
    }

    let mut pipeline = crate::Pipeline::new();
    pipeline.prev_ir = Some(ir);
    pipeline.last_snapshot = Some(snapshot);

    let query = fission_test_driver::SelectorQuery::label("Open").scoped(
        fission_test_driver::SelectorQuery::semantic_identifier("mounted.app"),
    );
    let record = resolve_selector_record(&pipeline, &ScrollStateMap::default(), &query)
        .expect("scoped selector should resolve");
    assert_eq!(record.node.identifier.as_deref(), Some("app.open"));
    assert_eq!(record.node.children, Vec::<String>::new());

    let app_record = resolve_selector_record(
        &pipeline,
        &ScrollStateMap::default(),
        &fission_test_driver::SelectorQuery::semantic_identifier("mounted.app"),
    )
    .expect("semantic parent should resolve");
    assert_eq!(app_record.node.children, vec![app_open.to_string()]);

    let missing_query = fission_test_driver::SelectorQuery::semantic_identifier("missing.control");
    let detailed =
        match resolve_selector_record(&pipeline, &ScrollStateMap::default(), &missing_query) {
            Err(error) => error,
            Ok(_) => panic!("missing selector should fail"),
        };
    let fission_test_driver::TestResponse::SelectorError { failure } = detailed else {
        panic!("missing selector should return a selector error");
    };
    assert!(!failure.candidates.is_empty());

    let quiet = match resolve_selector_record(
        &pipeline,
        &ScrollStateMap::default(),
        &missing_query.without_candidates(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("quiet missing selector should fail"),
    };
    let fission_test_driver::TestResponse::SelectorError { failure } = quiet else {
        panic!("quiet missing selector should return a selector error");
    };
    assert!(failure.candidates.is_empty());
}

#[test]
fn hidden_selector_prefers_the_responsive_branch_participating_in_layout() {
    let root = WidgetId::from_u128(30);
    let active = WidgetId::from_u128(31);
    let inactive = WidgetId::from_u128(32);
    let mut ir = CoreIR::new();
    for id in [active, inactive] {
        ir.add_node(
            id,
            Op::Semantics(Semantics {
                role: Role::Button,
                label: Some("Continue".into()),
                identifier: Some("tour.continue".into()),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
    }
    ir.add_node(
        root,
        Op::Semantics(Semantics {
            role: Role::Generic,
            identifier: Some("root".into()),
            ..Semantics::default()
        }),
        vec![active, inactive],
    );
    ir.set_root(root);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
    snapshot.nodes.insert(
        root,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            content_size: LayoutSize::new(320.0, 240.0),
        },
    );
    snapshot.nodes.insert(
        active,
        LayoutNodeGeometry {
            rect: LayoutRect::new(20.0, 30.0, 120.0, 40.0),
            content_size: LayoutSize::new(120.0, 40.0),
        },
    );

    let mut pipeline = crate::Pipeline::new();
    pipeline.prev_ir = Some(ir);
    pipeline.last_snapshot = Some(snapshot);
    let query =
        fission_test_driver::SelectorQuery::semantic_identifier("tour.continue").include_hidden();
    let record = resolve_selector_record(&pipeline, &ScrollStateMap::default(), &query)
        .expect("the active responsive branch should disambiguate the selector");

    assert_eq!(record.id, active);
}
