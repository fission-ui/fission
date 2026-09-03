use super::*;
use fission_core::{
    Action as FissionAction, ActionId as FissionActionId, ActionRegistry, GlobalState,
    ReducerContext, UpdateTextInput,
};
use fission_ir::{ActionEntry, ActionSet, CoreIR, CoreNode, Op};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutSize};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[test]
fn accessibility_character_geometry_comes_from_resolved_unicode_clusters() {
    let paragraph = ResolvedParagraphLayout {
        constraint_width: Some(40.0),
        size: LayoutSize::new(40.0, 16.0),
        lines: Vec::new(),
        inline_boxes: Vec::new(),
        clusters: vec![
            fission_layout::ParagraphCluster {
                start_index: 0,
                end_index: 1,
                line_index: 0,
                rect: LayoutRect::new(2.0, 0.0, 5.0, 16.0),
                is_rtl: false,
            },
            fission_layout::ParagraphCluster {
                start_index: 1,
                end_index: 3,
                line_index: 0,
                rect: LayoutRect::new(7.0, 0.0, 9.0, 16.0),
                is_rtl: false,
            },
        ],
        glyphs: Vec::new(),
        caret_stops: Vec::new(),
        selection_boxes: Vec::new(),
    };

    let (positions, widths) =
        paragraph_character_geometry(&paragraph, "aé", fission_ir::op::TextDirection::Ltr);

    assert_eq!(positions, vec![2.0, 7.0]);
    assert_eq!(widths, vec![5.0, 9.0]);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UpdateField(String);

impl FissionAction for UpdateField {
    fn static_id() -> FissionActionId {
        FissionActionId::from_name("accessibility_tests::UpdateField")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedContextualEdit {
    change: UpdateTextInput,
    scoped_target: Option<WidgetId>,
}

#[derive(Debug, Default)]
struct TextDispatchState {
    contextual: BTreeMap<(u128, String), RecordedContextualEdit>,
}

impl GlobalState for TextDispatchState {}

fn record_contextual_edit(
    state: &mut TextDispatchState,
    action: UpdateField,
    ctx: &mut ReducerContext<TextDispatchState>,
) {
    let change = if let Some(change) = ctx.input.text_change() {
        change.clone()
    } else {
        let selection = ctx
            .input
            .text_selection_change()
            .expect("contextual text or selection input");
        UpdateTextInput::from_values(
            selection.node_id,
            selection.value.clone(),
            selection.value.clone(),
            selection.source,
            fission_core::TextEditPhase::Selection,
        )
    };
    state.contextual.insert(
        (ctx.input.action_scope_id().unwrap_or_default(), action.0),
        RecordedContextualEdit {
            change,
            scoped_target: ctx.input.scoped_target(),
        },
    );
}

fn text_dispatch_runtime() -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TextDispatchState::default()))
        .expect("register text dispatch test state");
    let mut registry = ActionRegistry::<TextDispatchState>::new();
    registry.register(
        record_contextual_edit
            as fn(&mut TextDispatchState, UpdateField, &mut ReducerContext<TextDispatchState>),
    );
    runtime.absorb_registry(registry);
    runtime
}

fn contextual_text_semantics(field: &str) -> Semantics {
    Semantics {
        role: Role::TextInput,
        focusable: true,
        value: Some(String::new()),
        actions: ActionSet {
            entries: vec![ActionEntry {
                trigger: ActionTrigger::TextChanged,
                action_id: UpdateField::static_id().as_u128(),
                payload_data: Some(UpdateField(field.to_string()).encode()),
            }],
        },
        ..Semantics::default()
    }
}

fn dispatch_set_value_request(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    value: impl Into<Box<str>>,
) -> bool {
    dispatch_set_value_data(
        runtime,
        ir,
        &LayoutSnapshot::new(LayoutSize::new(320.0, 80.0)),
        target,
        ActionData::Value(value.into()),
    )
}

fn dispatch_set_value_data(
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    target: WidgetId,
    data: ActionData,
) -> bool {
    let access_node = NodeId((target.as_u128() as u64).max(2));
    let node_map = HashMap::from([(access_node, target)]);
    let request = ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: access_node,
        data: Some(data),
    };
    dispatch_mapped_accessibility_action(request, runtime, ir, layout, &node_map)
}

fn add_scoped_text_input(
    ir: &mut CoreIR,
    target: WidgetId,
    scope_node: WidgetId,
    scope_id: u128,
    semantics: Semantics,
) {
    add_node(ir, target, Op::Semantics(semantics), vec![]);
    add_node(
        ir,
        scope_node,
        Op::Semantics(Semantics {
            action_scope_id: Some(scope_id),
            ..Semantics::default()
        }),
        vec![target],
    );
}

fn add_node(ir: &mut CoreIR, id: WidgetId, op: Op, children: Vec<WidgetId>) {
    ir.nodes.insert(
        id,
        CoreNode {
            id,
            op,
            composite: Default::default(),
            children: children.clone(),
            parent: None,
            hash: 0,
        },
    );
    for child in children {
        ir.nodes.get_mut(&child).unwrap().parent = Some(id);
    }
}

#[test]
fn derives_button_label_from_descendant_text() {
    let root = WidgetId::from_u128(10);
    let button = WidgetId::from_u128(11);
    let text = WidgetId::from_u128(12);
    let mut ir = CoreIR::new();
    add_node(
        &mut ir,
        text,
        Op::Paint(PaintOp::DrawText {
            text: "Save".into(),
            size: 14.0,
            color: fission_ir::op::Color::BLACK,
            underline: false,
            locale: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        vec![],
    );
    add_node(
        &mut ir,
        button,
        Op::Semantics(Semantics {
            role: Role::Button,
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: 42,
                    payload_data: Some(Vec::new()),
                }],
            },
            focusable: true,
            ..Semantics::default()
        }),
        vec![text],
    );
    add_node(
        &mut ir,
        root,
        Op::Layout(fission_ir::LayoutOp::Box {
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
        }),
        vec![button],
    );
    ir.root = Some(root);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 50.0));
    layout.nodes.insert(
        button,
        LayoutNodeGeometry {
            rect: LayoutRect::new(10.0, 5.0, 80.0, 30.0),
            content_size: LayoutSize::new(80.0, 30.0),
        },
    );
    layout.nodes.insert(
        text,
        LayoutNodeGeometry {
            rect: LayoutRect {
                origin: LayoutPoint::new(12.0, 8.0),
                size: LayoutSize::new(40.0, 20.0),
            },
            content_size: LayoutSize::new(40.0, 20.0),
        },
    );

    let runtime = Runtime::default();
    let update = build_tree_update(&ir, &layout, &runtime, 2.0).update;
    let (_, node) = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == AccessRole::Button)
        .expect("button node");
    assert_eq!(node.label(), Some("Save"));
    assert!(node.supports_action(Action::Click));
    assert_eq!(node.bounds(), Some(Rect::new(20.0, 10.0, 180.0, 70.0)));
}

#[test]
fn maps_radio_semantics_to_accesskit_radio_button() {
    let semantics = Semantics {
        role: Role::Radio,
        checked: Some(true),
        ..Semantics::default()
    };

    assert_eq!(access_role_for(&semantics), AccessRole::RadioButton);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UpdateRange(String);

impl FissionAction for UpdateRange {
    fn static_id() -> FissionActionId {
        FissionActionId::from_name("accessibility_tests::UpdateRange")
    }
}

#[derive(Debug, Default)]
struct RangeDispatchState {
    field: Option<String>,
    changes: Vec<fission_core::RangeSliderChanged>,
}

impl GlobalState for RangeDispatchState {}

fn record_range_change(
    state: &mut RangeDispatchState,
    action: UpdateRange,
    ctx: &mut ReducerContext<RangeDispatchState>,
) {
    state.field = Some(action.0);
    state.changes.push(
        ctx.input
            .range_slider_change()
            .expect("range input")
            .clone(),
    );
}

fn accessible_range_slider() -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId) {
    let range_id = WidgetId::explicit("accessible-range");
    let widget: fission_core::Widget = fission_widgets::RangeSlider {
        id: Some(range_id),
        semantics_identifier: Some("filters.price".into()),
        start: 20.0,
        end: 80.0,
        min: 0.0,
        max: 100.0,
        step: Some(5.0),
        on_change: Some(ActionEnvelope {
            id: UpdateRange::static_id(),
            payload: UpdateRange("price".into()).encode(),
        }),
    }
    .into();
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let mut lowering = fission_core::internal::InternalLoweringCx::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.ir.root = Some(root);
    let input = fission_core::internal::build_layout_tree(&lowering.ir, &env);
    let mut engine = fission_layout::LayoutEngine::new();
    engine.rebuild(&input).unwrap();
    let layout = engine
        .compute_layout(&input, root, LayoutSize::new(400.0, 80.0), &|_| 0.0)
        .unwrap();
    let find = |identifier: &str| {
        lowering
            .ir
            .nodes
            .iter()
            .find_map(|(id, node)| match &node.op {
                Op::Semantics(semantics) if semantics.identifier.as_deref() == Some(identifier) => {
                    Some(*id)
                }
                _ => None,
            })
            .unwrap()
    };
    let start = find("filters.price.start");
    let end = find("filters.price.end");
    (lowering.ir, layout, start, end)
}

fn range_dispatch_runtime() -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(RangeDispatchState::default()))
        .unwrap();
    let mut registry = ActionRegistry::<RangeDispatchState>::new();
    registry.register(
        record_range_change
            as fn(&mut RangeDispatchState, UpdateRange, &mut ReducerContext<RangeDispatchState>),
    );
    runtime.absorb_registry(registry);
    runtime
}

#[test]
fn accesskit_range_thumbs_set_and_increment_with_contextual_input() {
    let (ir, layout, start, end) = accessible_range_slider();
    let runtime = Runtime::default();
    let update = build_tree_update(&ir, &layout, &runtime, 1.0).update;
    let slider_nodes = update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == AccessRole::Slider)
        .map(|(_, node)| node)
        .collect::<Vec<_>>();
    assert_eq!(slider_nodes.len(), 2);
    assert!(slider_nodes.iter().all(|node| {
        node.supports_action(Action::SetValue)
            && node.supports_action(Action::Increment)
            && node.supports_action(Action::Decrement)
    }));

    let mut runtime = range_dispatch_runtime();
    assert!(dispatch_set_value_data(
        &mut runtime,
        &ir,
        &layout,
        start,
        ActionData::NumericValue(35.0),
    ));

    let access_node = NodeId((end.as_u128() as u64).max(2));
    let node_map = HashMap::from([(access_node, end)]);
    assert!(dispatch_mapped_accessibility_action(
        ActionRequest {
            action: Action::Increment,
            target_tree: TreeId::ROOT,
            target_node: access_node,
            data: None,
        },
        &mut runtime,
        &ir,
        &layout,
        &node_map,
    ));

    let state = runtime.get_app_state::<RangeDispatchState>().unwrap();
    assert_eq!(state.field.as_deref(), Some("price"));
    assert_eq!(state.changes.len(), 2);
    assert_eq!((state.changes[0].start, state.changes[0].end), (35.0, 80.0));
    assert_eq!(
        state.changes[0].active_thumb,
        fission_core::RangeSliderThumb::Start
    );
    assert_eq!(
        state.changes[0].source,
        fission_core::RangeSliderChangeSource::Accessibility
    );
    assert_eq!((state.changes[1].start, state.changes[1].end), (20.0, 85.0));
    assert_eq!(
        state.changes[1].active_thumb,
        fission_core::RangeSliderThumb::End
    );
}

#[test]
fn text_input_value_prefers_lowered_semantics_over_retained_runtime_buffer() {
    let input = WidgetId::from_u128(20);
    let mut runtime = Runtime::default();
    runtime.runtime_state.text_edit.sync_from_runtime(
        input,
        "Stale retained buffer",
        None,
        None,
        false,
    );

    let semantics = Semantics {
        role: Role::TextInput,
        value: Some("Lowered model value".into()),
        ..Semantics::default()
    };
    assert_eq!(
        semantic_value(&runtime, input, &semantics).as_deref(),
        Some("Lowered model value")
    );

    let fallback_semantics = Semantics {
        role: Role::TextInput,
        ..Semantics::default()
    };
    assert_eq!(
        semantic_value(&runtime, input, &fallback_semantics).as_deref(),
        Some("Stale retained buffer")
    );
}

#[test]
fn accesskit_set_value_preserves_context_and_edit_geometry_across_fields() {
    let first = WidgetId::explicit("first-field");
    let second = WidgetId::explicit("second-field");
    let first_scope_node = WidgetId::explicit("first-scope");
    let second_scope_node = WidgetId::explicit("second-scope");
    let first_scope = 0xabc;
    let second_scope = 0xdef;
    let first_semantics = contextual_text_semantics("smtp_host");
    let second_semantics = contextual_text_semantics("smtp_port");
    let mut ir = CoreIR::new();
    add_scoped_text_input(
        &mut ir,
        first,
        first_scope_node,
        first_scope,
        first_semantics.clone(),
    );
    add_scoped_text_input(
        &mut ir,
        second,
        second_scope_node,
        second_scope,
        second_semantics.clone(),
    );
    let mut runtime = text_dispatch_runtime();

    assert!(dispatch_set_value_request(
        &mut runtime,
        &ir,
        first,
        "greenmail",
    ));
    assert!(dispatch_set_value_request(
        &mut runtime,
        &ir,
        second,
        "3025",
    ));

    let state = runtime
        .get_app_state::<TextDispatchState>()
        .expect("text dispatch state");
    let first_edit = state
        .contextual
        .get(&(first_scope, "smtp_host".into()))
        .expect("first contextual edit");
    assert_eq!(first_edit.change.node_id, first);
    assert_eq!(first_edit.change.new_text, "greenmail");
    assert_eq!(first_edit.change.new_caret, "greenmail".len());
    assert_eq!(first_edit.change.new_anchor, "greenmail".len());
    assert_eq!(first_edit.scoped_target, Some(first));
    let second_edit = state
        .contextual
        .get(&(second_scope, "smtp_port".into()))
        .expect("second contextual edit");
    assert_eq!(second_edit.change.node_id, second);
    assert_eq!(second_edit.change.new_text, "3025");
    assert_eq!(second_edit.change.new_caret, 4);
    assert_eq!(second_edit.change.new_anchor, 4);
    assert_eq!(second_edit.scoped_target, Some(second));
}

#[test]
fn native_ime_and_accesskit_set_value_share_the_text_edit_contract() {
    let target = WidgetId::explicit("ime-accessibility-parity");
    let scope_node = WidgetId::explicit("ime-accessibility-scope");
    let scope_id = 0x717;
    let semantics = contextual_text_semantics("display_name");
    let mut ir = CoreIR::new();
    add_scoped_text_input(&mut ir, target, scope_node, scope_id, semantics.clone());
    ir.root = Some(scope_node);
    let layout = LayoutSnapshot::new(LayoutSize::new(320.0, 80.0));

    let mut native_runtime = text_dispatch_runtime();
    native_runtime
        .runtime_state
        .interaction
        .set_focused(Some(target));
    native_runtime
        .handle_input(
            InputEvent::Ime(ImeEvent::Commit {
                text: "café".into(),
            }),
            &ir,
            &layout,
        )
        .expect("native IME dispatch");
    let native_edit = native_runtime
        .get_app_state::<TextDispatchState>()
        .and_then(|state| state.contextual.get(&(scope_id, "display_name".into())))
        .cloned()
        .expect("native contextual edit");

    let mut accessibility_runtime = text_dispatch_runtime();
    assert!(dispatch_set_value_request(
        &mut accessibility_runtime,
        &ir,
        target,
        "café",
    ));
    let accessibility_edit = accessibility_runtime
        .get_app_state::<TextDispatchState>()
        .and_then(|state| state.contextual.get(&(scope_id, "display_name".into())))
        .cloned()
        .expect("accessibility contextual edit");

    assert_eq!(
        native_edit.change.node_id,
        accessibility_edit.change.node_id
    );
    assert_eq!(
        native_edit.change.new_text,
        accessibility_edit.change.new_text
    );
    assert_eq!(
        native_edit.change.new_value,
        accessibility_edit.change.new_value
    );
    assert_eq!(native_edit.scoped_target, accessibility_edit.scoped_target);
    assert_eq!(native_edit.change.source, fission_core::TextEditSource::Ime);
    assert_eq!(
        accessibility_edit.change.source,
        fission_core::TextEditSource::Accessibility
    );
    assert_eq!(native_edit.change.new_caret, "café".len());
    assert_eq!(native_edit.change.new_anchor, "café".len());
}

#[test]
fn identically_named_contextual_fields_remain_isolated_by_scope() {
    let first = WidgetId::explicit("account-one-host");
    let second = WidgetId::explicit("account-two-host");
    let first_scope = 0x111;
    let second_scope = 0x222;
    let semantics = contextual_text_semantics("host");
    let mut ir = CoreIR::new();
    add_scoped_text_input(
        &mut ir,
        first,
        WidgetId::explicit("account-one"),
        first_scope,
        semantics.clone(),
    );
    add_scoped_text_input(
        &mut ir,
        second,
        WidgetId::explicit("account-two"),
        second_scope,
        semantics.clone(),
    );
    let mut runtime = text_dispatch_runtime();

    assert!(dispatch_set_value_request(
        &mut runtime,
        &ir,
        first,
        "mail-one",
    ));
    assert!(dispatch_set_value_request(
        &mut runtime,
        &ir,
        second,
        "mail-two",
    ));

    let state = runtime
        .get_app_state::<TextDispatchState>()
        .expect("text dispatch state");
    assert_eq!(
        state
            .contextual
            .get(&(first_scope, "host".into()))
            .map(|edit| edit.change.new_text.as_str()),
        Some("mail-one")
    );
    assert_eq!(
        state
            .contextual
            .get(&(second_scope, "host".into()))
            .map(|edit| edit.change.new_text.as_str()),
        Some("mail-two")
    );
}

#[test]
fn accesskit_numeric_text_input_preserves_context_and_transitional_text() {
    let number = WidgetId::explicit("numeric-text-input");
    let scope_node = WidgetId::explicit("numeric-text-scope");
    let scope_id = 0x515;
    let mut number_semantics = contextual_text_semantics("retry_count");
    number_semantics.text_input_type = TextInputType::Number;
    let mut ir = CoreIR::new();
    add_scoped_text_input(
        &mut ir,
        number,
        scope_node,
        scope_id,
        number_semantics.clone(),
    );
    let mut runtime = text_dispatch_runtime();

    assert!(dispatch_set_value_request(&mut runtime, &ir, number, "-",));
    assert_eq!(
        runtime
            .get_app_state::<TextDispatchState>()
            .and_then(|state| { state.contextual.get(&(scope_id, "retry_count".into())) })
            .map(|edit| edit.change.new_text.as_str()),
        Some("-")
    );
    assert!(dispatch_set_value_data(
        &mut runtime,
        &ir,
        &LayoutSnapshot::new(LayoutSize::new(320.0, 80.0)),
        number,
        ActionData::NumericValue(12.5),
    ));

    let state = runtime
        .get_app_state::<TextDispatchState>()
        .expect("text dispatch state");
    let edit = state
        .contextual
        .get(&(scope_id, "retry_count".into()))
        .expect("numeric contextual edit");
    assert_eq!(edit.change.new_text, "12.5");
    assert_eq!(edit.change.node_id, number);
    assert_eq!(edit.scoped_target, Some(number));
}

#[test]
fn accesskit_rejects_and_does_not_advertise_edits_for_disabled_or_read_only_inputs() {
    let disabled = WidgetId::explicit("disabled-text-input");
    let read_only = WidgetId::explicit("read-only-text-input");
    let root = WidgetId::explicit("text-input-root");
    let mut disabled_semantics = contextual_text_semantics("disabled");
    disabled_semantics.disabled = true;
    let mut read_only_semantics = contextual_text_semantics("read_only");
    read_only_semantics.read_only = true;
    let mut ir = CoreIR::new();
    add_node(
        &mut ir,
        disabled,
        Op::Semantics(disabled_semantics.clone()),
        vec![],
    );
    add_node(
        &mut ir,
        read_only,
        Op::Semantics(read_only_semantics.clone()),
        vec![],
    );
    add_node(
        &mut ir,
        root,
        Op::Semantics(Semantics::default()),
        vec![disabled, read_only],
    );
    ir.root = Some(root);
    let mut runtime = text_dispatch_runtime();

    assert!(!dispatch_set_value_request(
        &mut runtime,
        &ir,
        disabled,
        "must-not-dispatch",
    ));
    assert!(!dispatch_set_value_request(
        &mut runtime,
        &ir,
        read_only,
        "must-not-dispatch",
    ));
    assert!(runtime
        .get_app_state::<TextDispatchState>()
        .expect("text dispatch state")
        .contextual
        .is_empty());
    assert!(runtime.runtime_state.text_edit.get(disabled).is_none());
    assert!(runtime.runtime_state.text_edit.get(read_only).is_none());

    let layout = LayoutSnapshot::new(LayoutSize::new(320.0, 80.0));
    let update = build_tree_update(&ir, &layout, &runtime, 1.0).update;
    let text_inputs = update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == AccessRole::TextInput)
        .map(|(_, node)| node)
        .collect::<Vec<_>>();
    assert_eq!(text_inputs.len(), 2);
    assert!(text_inputs.iter().all(|node| {
        !node.supports_action(Action::SetValue)
            && !node.supports_action(Action::ReplaceSelectedText)
    }));
}

#[test]
fn accessibility_selection_dispatches_unicode_byte_offsets_in_scope() {
    let target = WidgetId::explicit("unicode-selection");
    let scope_node = WidgetId::explicit("unicode-scope");
    let scope_id = 0x404;
    let semantics = Semantics {
        role: Role::TextInput,
        focusable: true,
        value: Some("aé🦀z".into()),
        actions: ActionSet {
            entries: vec![ActionEntry {
                trigger: ActionTrigger::CursorChange,
                action_id: UpdateField::static_id().as_u128(),
                payload_data: Some(UpdateField("selection".into()).encode()),
            }],
        },
        ..Semantics::default()
    };
    let mut ir = CoreIR::new();
    add_scoped_text_input(&mut ir, target, scope_node, scope_id, semantics.clone());
    let mut runtime = text_dispatch_runtime();
    let access_node = NodeId(77);
    let selection = TextSelection {
        anchor: TextPosition {
            node: access_node,
            character_index: 1,
        },
        focus: TextPosition {
            node: access_node,
            character_index: 3,
        },
    };

    assert!(set_text_selection(
        &mut runtime,
        &ir,
        &LayoutSnapshot::default(),
        target,
        &semantics,
        &selection,
    ));

    let state = runtime
        .get_app_state::<TextDispatchState>()
        .expect("text dispatch state");
    let recorded = state
        .contextual
        .get(&(scope_id, "selection".into()))
        .expect("contextual selection action");
    assert_eq!(recorded.change.new_caret, 7);
    assert_eq!(recorded.change.new_anchor, 1);
    assert_eq!(
        recorded.change.source,
        fission_core::TextEditSource::Accessibility
    );
    assert_eq!(
        recorded.change.phase,
        fission_core::TextEditPhase::Selection
    );
    assert_eq!(recorded.scoped_target, Some(target));
}

#[test]
fn accessibility_selection_updates_a_coordinated_read_only_region() {
    let region = WidgetId::explicit("accessible-selection-region");
    let first = WidgetId::explicit("accessible-selection-first");
    let second = WidgetId::explicit("accessible-selection-second");
    let semantics = Semantics {
        role: Role::Text,
        value: Some("aé\n🦀z".into()),
        focusable: true,
        read_only: true,
        selection_region: Some(fission_ir::SelectionRegionSemantics {
            excluded: false,
            separator: "\n".into(),
        }),
        ..Semantics::default()
    };
    let mut ir = CoreIR::new();
    for (id, value) in [(first, "aé"), (second, "🦀z")] {
        add_node(
            &mut ir,
            id,
            Op::Semantics(Semantics {
                role: Role::Text,
                value: Some(value.into()),
                selectable_text: true,
                read_only: true,
                ..Semantics::default()
            }),
            vec![],
        );
    }
    add_node(
        &mut ir,
        region,
        Op::Semantics(semantics.clone()),
        vec![first, second],
    );
    ir.root = Some(region);
    let mut runtime = Runtime::default();
    let selection = TextSelection {
        anchor: TextPosition {
            node: NodeId(77),
            character_index: 1,
        },
        focus: TextPosition {
            node: NodeId(77),
            character_index: 4,
        },
    };

    assert!(set_text_selection(
        &mut runtime,
        &ir,
        &LayoutSnapshot::default(),
        region,
        &semantics,
        &selection,
    ));
    let selected = SelectionRegionController::new(region)
        .selection(&runtime.runtime_state)
        .expect("region selection");
    assert_eq!(selected.base.node_id, first);
    assert_eq!(selected.base.offset.utf8_offset(), 1);
    assert_eq!(selected.extent.node_id, second);
    assert_eq!(selected.extent.offset.utf8_offset(), "🦀".len());

    let layout = LayoutSnapshot::new(LayoutSize::new(320.0, 80.0));
    let update = build_tree_update(&ir, &layout, &runtime, 1.0).update;
    let region_node = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.value().as_deref() == Some("aé\n🦀z")).then_some(node))
        .expect("selection region accessibility node");
    assert!(region_node.supports_action(Action::SetTextSelection));
    assert!(region_node.children().is_empty());
}

#[test]
fn text_input_accessibility_exposes_validation_without_masked_value() {
    let target = WidgetId::explicit("validated-password");
    let semantics = Semantics {
        role: Role::TextInput,
        label: Some("Password".into()),
        value: Some("do-not-expose".into()),
        masked: true,
        required: true,
        validation_state: fission_ir::semantics::TextFieldValidationState::Invalid,
        validation_message: Some("Password is required".into()),
        ..Semantics::default()
    };
    let mut ir = CoreIR::new();
    add_node(&mut ir, target, Op::Semantics(semantics), vec![]);
    ir.root = Some(target);
    let runtime = text_dispatch_runtime();
    let layout = LayoutSnapshot::new(LayoutSize::new(320.0, 80.0));

    let update = build_tree_update(&ir, &layout, &runtime, 1.0).update;
    let node = update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.role() == AccessRole::PasswordInput)
        .expect("password input node");

    assert!(node.is_required());
    assert_eq!(node.invalid(), Some(AccessInvalid::True));
    assert_eq!(node.description(), Some("Password is required"));
    assert_eq!(node.live(), Some(Live::Polite));
    assert!(node.is_live_atomic());
    assert_eq!(node.value(), None);
}

#[test]
fn accesskit_set_value_reports_dispatch_failure_and_restores_editor_state() {
    let target = WidgetId::explicit("bad-contextual-payload");
    let semantics = Semantics {
        role: Role::TextInput,
        actions: ActionSet {
            entries: vec![ActionEntry {
                trigger: ActionTrigger::TextChanged,
                action_id: UpdateField::static_id().as_u128(),
                payload_data: Some(b"not-json".to_vec()),
            }],
        },
        ..Semantics::default()
    };
    let mut ir = CoreIR::new();
    add_node(&mut ir, target, Op::Semantics(semantics.clone()), vec![]);
    let mut runtime = text_dispatch_runtime();

    assert!(!dispatch_set_value_request(
        &mut runtime,
        &ir,
        target,
        "value",
    ));
    let state = runtime
        .get_app_state::<TextDispatchState>()
        .expect("text dispatch state");
    assert!(state.contextual.is_empty());
    assert!(runtime.runtime_state.text_edit.get(target).is_none());
}
