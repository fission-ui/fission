#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::{Arc, Mutex};

    use accesskit::{
        Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler,
        Invalid as AccessInvalid, Live, Node, NodeId, Rect, Role as AccessRole,
        TextDirection as AccessTextDirection, TextPosition, TextSelection, Toggled, Tree, TreeId,
        TreeUpdate,
    };
    use accesskit_winit::Adapter;
    #[cfg(test)]
    use fission_core::event::ImeEvent;
    #[cfg(test)]
    use fission_core::InputEvent;
    use fission_core::{
        ActionEnvelope, ActionId, ActionInput, Runtime, SelectionRegionController, TextAffinity,
        TextPosition as EditingTextPosition,
    };
    use fission_ir::semantics::{ActionTrigger, Role, TextInputType};
    use fission_ir::{CoreIR, LayoutOp, Op, PaintOp, Semantics, WidgetId};
    use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot, ResolvedParagraphLayout};
    use fission_test_driver::TestEvent;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    const ROOT_NODE_ID: NodeId = NodeId(1);

    #[derive(Debug)]
    enum QueuedAccessibilityEvent {
        ActionRequested(ActionRequest),
        Deactivated,
    }

    struct AccessibilityShared {
        latest_update: Mutex<TreeUpdate>,
        latest_node_map: Mutex<HashMap<NodeId, WidgetId>>,
        events: Mutex<VecDeque<QueuedAccessibilityEvent>>,
        proxy: EventLoopProxy<TestEvent>,
    }

    impl AccessibilityShared {
        fn wake(&self) {
            let _ = self.proxy.send_event(TestEvent::Wake);
        }
    }

    struct FissionActivationHandler {
        shared: Arc<AccessibilityShared>,
    }

    impl ActivationHandler for FissionActivationHandler {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            self.shared
                .latest_update
                .lock()
                .ok()
                .map(|update| update.clone())
        }
    }

    struct FissionActionHandler {
        shared: Arc<AccessibilityShared>,
    }

    impl ActionHandler for FissionActionHandler {
        fn do_action(&mut self, request: ActionRequest) {
            if let Ok(mut events) = self.shared.events.lock() {
                events.push_back(QueuedAccessibilityEvent::ActionRequested(request));
            }
            self.shared.wake();
        }
    }

    struct FissionDeactivationHandler {
        shared: Arc<AccessibilityShared>,
    }

    impl DeactivationHandler for FissionDeactivationHandler {
        fn deactivate_accessibility(&mut self) {
            if let Ok(mut events) = self.shared.events.lock() {
                events.push_back(QueuedAccessibilityEvent::Deactivated);
            }
            self.shared.wake();
        }
    }

    pub struct AccessibilityBridge {
        adapter: Option<Adapter>,
        shared: Arc<AccessibilityShared>,
        active: bool,
    }

    impl AccessibilityBridge {
        pub fn new(proxy: EventLoopProxy<TestEvent>) -> Self {
            Self {
                adapter: None,
                shared: Arc::new(AccessibilityShared {
                    latest_update: Mutex::new(placeholder_update()),
                    latest_node_map: Mutex::new(HashMap::new()),
                    events: Mutex::new(VecDeque::new()),
                    proxy,
                }),
                active: false,
            }
        }

        pub fn ensure_adapter(&mut self, event_loop: &ActiveEventLoop, window: &Window) {
            if self.adapter.is_some() {
                return;
            }
            let activation_handler = FissionActivationHandler {
                shared: self.shared.clone(),
            };
            let action_handler = FissionActionHandler {
                shared: self.shared.clone(),
            };
            let deactivation_handler = FissionDeactivationHandler {
                shared: self.shared.clone(),
            };
            self.adapter = Some(Adapter::with_direct_handlers(
                event_loop,
                window,
                activation_handler,
                action_handler,
                deactivation_handler,
            ));
        }

        pub fn process_window_event(&mut self, window: &Window, event: &WindowEvent) {
            if let Some(adapter) = self.adapter.as_mut() {
                adapter.process_event(window, event);
            }
        }

        pub fn update_tree(
            &mut self,
            ir: &CoreIR,
            layout: &LayoutSnapshot,
            runtime: &Runtime,
            scale_factor: f64,
        ) {
            let built = build_tree_update(ir, layout, runtime, scale_factor);
            if let Ok(mut latest) = self.shared.latest_update.lock() {
                *latest = built.update.clone();
            }
            if let Ok(mut node_map) = self.shared.latest_node_map.lock() {
                *node_map = built.node_map;
            }
            if let Some(adapter) = self.adapter.as_mut() {
                let update = built.update;
                adapter.update_if_active(|| update);
            }
        }

        pub fn drain_events(
            &mut self,
            runtime: &mut Runtime,
            ir: Option<&CoreIR>,
            layout: Option<&LayoutSnapshot>,
        ) -> bool {
            let mut changed = false;
            loop {
                let event = self
                    .shared
                    .events
                    .lock()
                    .ok()
                    .and_then(|mut events| events.pop_front());
                let Some(event) = event else {
                    break;
                };
                match event {
                    QueuedAccessibilityEvent::ActionRequested(request) => {
                        let Some(ir) = ir else {
                            continue;
                        };
                        let Some(layout) = layout else {
                            continue;
                        };
                        if self.handle_action_request(request, runtime, ir, layout) {
                            changed = true;
                        }
                    }
                    QueuedAccessibilityEvent::Deactivated => {
                        self.active = false;
                    }
                }
            }
            changed
        }

        fn handle_action_request(
            &mut self,
            request: ActionRequest,
            runtime: &mut Runtime,
            ir: &CoreIR,
            layout: &LayoutSnapshot,
        ) -> bool {
            self.active = true;
            let node_map = self.shared.latest_node_map.lock().ok();
            node_map.as_ref().is_some_and(|node_map| {
                dispatch_mapped_accessibility_action(request, runtime, ir, layout, node_map)
            })
        }
    }

    fn dispatch_mapped_accessibility_action(
        request: ActionRequest,
        runtime: &mut Runtime,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        node_map: &HashMap<NodeId, WidgetId>,
    ) -> bool {
        let Some(target) = node_map.get(&request.target_node).copied() else {
            return false;
        };
        let Some(semantics) = semantics_for(ir, target) else {
            return false;
        };
        dispatch_accessibility_action(request, runtime, ir, layout, target, semantics)
    }

    fn dispatch_accessibility_action(
        request: ActionRequest,
        runtime: &mut Runtime,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        target: WidgetId,
        semantics: &Semantics,
    ) -> bool {
        match request.action {
            Action::Click => dispatch_semantics_action(
                runtime,
                ir,
                target,
                semantics,
                ActionTrigger::Default,
                ActionInput::None,
            ),
            Action::Focus => set_focus(runtime, ir, Some(target)),
            Action::Blur => set_focus(runtime, ir, None),
            Action::ReplaceSelectedText => {
                if !editable_text_input(semantics) || !has_text_input_action(semantics) {
                    crate::log_input_dispatch_failure(
                        "accessibility_replace_selected_text_rejected",
                        Some(target),
                    );
                    return false;
                }
                let Some(text) = value_action_data(&request.data) else {
                    return false;
                };
                set_focus(runtime, ir, Some(target));
                let value = runtime
                    .runtime_state
                    .text_edit
                    .get(target)
                    .map(|state| state.editing_value())
                    .unwrap_or_else(|| {
                        fission_core::TextEditingValue::from_text(
                            semantics.value.clone().unwrap_or_default(),
                        )
                    });
                runtime
                    .apply_text_edit_command_to(
                        ir,
                        layout,
                        target,
                        fission_core::TextEditCommand::Replace {
                            range: value.selection_range(),
                            text: text.to_string(),
                            source: fission_core::TextEditSource::Accessibility,
                        },
                    )
                    .unwrap_or(false)
            }
            Action::SetValue => match &request.data {
                Some(ActionData::Value(value)) => {
                    set_text_input_value(runtime, ir, layout, target, semantics, value)
                }
                Some(ActionData::NumericValue(value)) if semantics.role == Role::TextInput => {
                    set_text_input_value(runtime, ir, layout, target, semantics, &value.to_string())
                }
                Some(ActionData::NumericValue(value)) => set_numeric_value(
                    runtime,
                    ir,
                    target,
                    semantics,
                    (*value as f32).clamp(
                        semantics.min_value.unwrap_or(f32::NEG_INFINITY),
                        semantics.max_value.unwrap_or(f32::INFINITY),
                    ),
                ),
                _ => false,
            },
            Action::SetTextSelection => {
                let Some(ActionData::SetTextSelection(selection)) = &request.data else {
                    return false;
                };
                set_text_selection(runtime, ir, layout, target, semantics, selection)
            }
            Action::ScrollDown | Action::ScrollUp | Action::ScrollLeft | Action::ScrollRight => {
                handle_scroll_action(runtime, ir, layout, target, request.action, &request.data)
            }
            Action::Increment => adjust_numeric_value(runtime, ir, target, semantics, 1.0),
            Action::Decrement => adjust_numeric_value(runtime, ir, target, semantics, -1.0),
            _ => false,
        }
    }

    fn editable_text_input(semantics: &Semantics) -> bool {
        semantics.role == Role::TextInput && !semantics.disabled && !semantics.read_only
    }

    fn has_text_input_action(semantics: &Semantics) -> bool {
        semantics
            .actions
            .entries
            .iter()
            .any(|entry| entry.trigger == ActionTrigger::TextChanged)
    }

    pub fn window_must_start_hidden() -> bool {
        true
    }

    fn placeholder_update() -> TreeUpdate {
        let mut root = Node::new(AccessRole::Window);
        root.set_bounds(Rect::ZERO);
        TreeUpdate {
            nodes: vec![(ROOT_NODE_ID, root)],
            tree: Some(Tree {
                root: ROOT_NODE_ID,
                toolkit_name: Some("Fission".to_string()),
                toolkit_version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
            }),
            tree_id: TreeId::ROOT,
            focus: ROOT_NODE_ID,
        }
    }

    struct BuiltTreeUpdate {
        update: TreeUpdate,
        node_map: HashMap<NodeId, WidgetId>,
    }

    fn build_tree_update(
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        runtime: &Runtime,
        scale_factor: f64,
    ) -> BuiltTreeUpdate {
        let mut builder = TreeUpdateBuilder::new(ir, layout, runtime, scale_factor);
        let root_children = ir
            .root
            .map(|root| builder.collect_subtree(root, false))
            .unwrap_or_default();

        let mut root = Node::new(AccessRole::Window);
        root.set_bounds(Rect::new(
            0.0,
            0.0,
            layout.viewport_size.width as f64 * scale_factor,
            layout.viewport_size.height as f64 * scale_factor,
        ));
        root.set_children(root_children);
        builder.nodes.push((ROOT_NODE_ID, root));

        let focus = runtime
            .runtime_state
            .interaction
            .focused
            .and_then(|id| builder.widget_to_node.get(&id).copied())
            .unwrap_or(ROOT_NODE_ID);

        BuiltTreeUpdate {
            update: TreeUpdate {
                nodes: builder.nodes,
                tree: Some(Tree {
                    root: ROOT_NODE_ID,
                    toolkit_name: Some("Fission".to_string()),
                    toolkit_version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
                }),
                tree_id: TreeId::ROOT,
                focus,
            },
            node_map: builder.node_to_widget,
        }
    }

    struct TreeUpdateBuilder<'a> {
        ir: &'a CoreIR,
        layout: &'a LayoutSnapshot,
        runtime: &'a Runtime,
        scale_factor: f64,
        nodes: Vec<(NodeId, Node)>,
        used_node_ids: HashSet<NodeId>,
        widget_to_node: HashMap<WidgetId, NodeId>,
        node_to_widget: HashMap<NodeId, WidgetId>,
    }

    impl<'a> TreeUpdateBuilder<'a> {
        fn new(
            ir: &'a CoreIR,
            layout: &'a LayoutSnapshot,
            runtime: &'a Runtime,
            scale_factor: f64,
        ) -> Self {
            let mut used_node_ids = HashSet::new();
            used_node_ids.insert(ROOT_NODE_ID);
            Self {
                ir,
                layout,
                runtime,
                scale_factor,
                nodes: Vec::new(),
                used_node_ids,
                widget_to_node: HashMap::new(),
                node_to_widget: HashMap::new(),
            }
        }

        fn collect_subtree(&mut self, node_id: WidgetId, inside_semantics: bool) -> Vec<NodeId> {
            let Some(core_node) = self.ir.nodes.get(&node_id) else {
                return Vec::new();
            };

            match &core_node.op {
                Op::Semantics(semantics) if include_semantics(semantics) => {
                    let coordinated_selection = semantics
                        .selection_region
                        .as_ref()
                        .is_some_and(|region| !region.excluded);
                    let child_ids = core_node
                        .children
                        .iter()
                        .flat_map(|child| {
                            if coordinated_selection {
                                self.collect_selection_region_subtree(*child)
                            } else {
                                self.collect_subtree(*child, true)
                            }
                        })
                        .collect::<Vec<_>>();
                    let access_id = self.node_id_for(node_id);
                    let mut node = self.access_node_for_semantics(access_id, node_id, semantics);
                    node.set_children(child_ids);
                    self.nodes.push((access_id, node));
                    vec![access_id]
                }
                Op::Paint(PaintOp::DrawText { text, .. }) if !text.is_empty() => {
                    let access_id = self.node_id_for(node_id);
                    let mut node = Node::new(AccessRole::Label);
                    node.set_value(text.clone());
                    self.apply_resolved_text_geometry(&mut node, node_id, text);
                    if let Some(rect) = self.visual_rect(node_id) {
                        node.set_bounds(accesskit_rect(rect, self.scale_factor));
                    }
                    if inside_semantics {
                        node.set_read_only();
                    }
                    self.nodes.push((access_id, node));
                    vec![access_id]
                }
                Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                    let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                    if text.is_empty() {
                        return Vec::new();
                    }
                    let access_id = self.node_id_for(node_id);
                    let mut node = Node::new(AccessRole::Label);
                    node.set_value(text.clone());
                    self.apply_resolved_text_geometry(&mut node, node_id, &text);
                    if let Some(rect) = self.visual_rect(node_id) {
                        node.set_bounds(accesskit_rect(rect, self.scale_factor));
                    }
                    if inside_semantics {
                        node.set_read_only();
                    }
                    self.nodes.push((access_id, node));
                    vec![access_id]
                }
                _ => core_node
                    .children
                    .iter()
                    .flat_map(|child| self.collect_subtree(*child, inside_semantics))
                    .collect(),
            }
        }

        /// Collects meaningful non-text descendants while the containing
        /// selection region represents all participating text as one node.
        fn collect_selection_region_subtree(&mut self, node_id: WidgetId) -> Vec<NodeId> {
            let Some(core_node) = self.ir.nodes.get(&node_id) else {
                return Vec::new();
            };
            match &core_node.op {
                Op::Semantics(semantics)
                    if semantics
                        .selection_region
                        .as_ref()
                        .is_some_and(|region| region.excluded) =>
                {
                    self.collect_subtree(node_id, true)
                }
                Op::Semantics(semantics) if semantics.selectable_text => core_node
                    .children
                    .iter()
                    .flat_map(|child| self.collect_selection_region_subtree(*child))
                    .collect(),
                Op::Paint(PaintOp::DrawText { .. } | PaintOp::DrawRichText { .. }) => Vec::new(),
                Op::Semantics(semantics) if include_semantics(semantics) => {
                    self.collect_subtree(node_id, true)
                }
                _ => core_node
                    .children
                    .iter()
                    .flat_map(|child| self.collect_selection_region_subtree(*child))
                    .collect(),
            }
        }

        fn node_id_for(&mut self, widget_id: WidgetId) -> NodeId {
            if let Some(node_id) = self.widget_to_node.get(&widget_id) {
                return *node_id;
            }
            let raw = widget_id.as_u128();
            let mut candidate = NodeId(((raw >> 64) as u64) ^ raw as u64);
            if candidate.0 <= ROOT_NODE_ID.0 {
                candidate.0 = candidate.0.saturating_add(2);
            }
            while self.used_node_ids.contains(&candidate) {
                candidate.0 = candidate.0.wrapping_add(1).max(2);
            }
            self.used_node_ids.insert(candidate);
            self.widget_to_node.insert(widget_id, candidate);
            self.node_to_widget.insert(candidate, widget_id);
            candidate
        }

        fn access_node_for_semantics(
            &self,
            access_id: NodeId,
            node_id: WidgetId,
            semantics: &Semantics,
        ) -> Node {
            let mut node = Node::new(access_role_for(semantics));
            if let Some(rect) = self.visual_rect(node_id) {
                node.set_bounds(accesskit_rect(rect, self.scale_factor));
            }
            if let Some(identifier) = semantics.identifier.as_deref() {
                node.set_author_id(identifier);
            }
            let value = semantic_value(self.runtime, node_id, semantics);
            let label = semantics
                .label
                .clone()
                .or_else(|| collect_descendant_text(self.ir, node_id));

            match semantics.role {
                Role::Text => {
                    let selection_region = semantics
                        .selection_region
                        .as_ref()
                        .is_some_and(|region| !region.excluded);
                    let text = if selection_region {
                        value.clone().or(label)
                    } else {
                        label.or(value.clone())
                    };
                    if let Some(text) = text {
                        node.set_value(text.clone());
                        self.apply_resolved_text_geometry(&mut node, node_id, &text);
                        if selection_region {
                            node.set_character_lengths(
                                text.chars()
                                    .map(|ch| ch.len_utf8() as u8)
                                    .collect::<Vec<_>>(),
                            );
                            if let Some((anchor, focus)) = semantics.text_selection {
                                node.set_text_selection(TextSelection {
                                    anchor: TextPosition {
                                        node: access_id,
                                        character_index: byte_to_char(&text, anchor),
                                    },
                                    focus: TextPosition {
                                        node: access_id,
                                        character_index: byte_to_char(&text, focus),
                                    },
                                });
                            }
                            if !semantics.disabled {
                                node.add_action(Action::SetTextSelection);
                            }
                        }
                    }
                    node.set_read_only();
                }
                Role::TextInput => {
                    if let Some(label) = label {
                        node.set_label(label);
                    }
                    if semantics.required {
                        node.set_required();
                    }
                    if matches!(
                        semantics.validation_state,
                        fission_ir::semantics::TextFieldValidationState::Invalid
                    ) {
                        node.set_invalid(AccessInvalid::True);
                    }
                    if let Some(message) = semantics.validation_message.as_deref() {
                        node.set_description(message);
                        if matches!(
                            semantics.validation_state,
                            fission_ir::semantics::TextFieldValidationState::Invalid
                        ) {
                            node.set_live(Live::Polite);
                            node.set_live_atomic();
                        }
                    }
                    if let Some(value) = value {
                        node.set_value(value.clone());
                        self.apply_resolved_text_geometry(&mut node, node_id, &value);
                        node.set_character_lengths(
                            value
                                .chars()
                                .map(|ch| ch.len_utf8() as u8)
                                .collect::<Vec<_>>(),
                        );
                        if let Some((anchor, focus)) = semantics.text_selection {
                            node.set_text_selection(TextSelection {
                                anchor: TextPosition {
                                    node: access_id,
                                    character_index: byte_to_char(&value, anchor),
                                },
                                focus: TextPosition {
                                    node: access_id,
                                    character_index: byte_to_char(&value, focus),
                                },
                            });
                        }
                    }
                    if editable_text_input(semantics) && has_text_input_action(semantics) {
                        node.add_action(Action::ReplaceSelectedText);
                        node.add_action(Action::SetValue);
                    }
                    if !semantics.disabled {
                        node.add_action(Action::SetTextSelection);
                    }
                    if semantics.read_only {
                        node.set_read_only();
                    }
                }
                _ => {
                    if let Some(label) = label {
                        node.set_label(label);
                    }
                    if let Some(value) = value {
                        node.set_value(value);
                    }
                }
            }

            if semantics.focusable && !semantics.disabled {
                node.add_action(Action::Focus);
                node.add_action(Action::Blur);
            }
            if semantics.disabled {
                node.set_disabled();
            }
            if let Some(checked) = semantics.checked {
                node.set_toggled(Toggled::from(checked));
            }
            if let Some(min) = semantics.min_value {
                node.set_min_numeric_value(min as f64);
            }
            if let Some(max) = semantics.max_value {
                node.set_max_numeric_value(max as f64);
            }
            if let Some(current) = semantics.current_value {
                node.set_numeric_value(current as f64);
            }
            if semantics.current_value.is_some() {
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
                node.add_action(Action::SetValue);
            }
            if semantics.scrollable_y {
                node.add_action(Action::ScrollDown);
                node.add_action(Action::ScrollUp);
                if let Some((offset, max)) =
                    scroll_offset_and_max(self.ir, self.layout, self.runtime, node_id, false)
                {
                    node.set_scroll_y(offset as f64);
                    node.set_scroll_y_min(0.0);
                    node.set_scroll_y_max(max as f64);
                }
            }
            if semantics.scrollable_x {
                node.add_action(Action::ScrollLeft);
                node.add_action(Action::ScrollRight);
                if let Some((offset, max)) =
                    scroll_offset_and_max(self.ir, self.layout, self.runtime, node_id, true)
                {
                    node.set_scroll_x(offset as f64);
                    node.set_scroll_x_min(0.0);
                    node.set_scroll_x_max(max as f64);
                }
            }
            if semantics
                .actions
                .entries
                .iter()
                .any(|entry| entry.trigger == ActionTrigger::Default)
                && !semantics.disabled
            {
                node.add_action(Action::Click);
            }
            node
        }

        fn apply_resolved_text_geometry(&self, node: &mut Node, node_id: WidgetId, text: &str) {
            let Some((paragraph, direction)) =
                resolved_paragraph_in_subtree(self.ir, self.layout, node_id, text.len())
            else {
                return;
            };
            let (positions, widths) = paragraph_character_geometry(paragraph, text, direction);
            if positions.len() != text.chars().count() {
                return;
            }
            node.set_character_lengths(
                text.chars()
                    .map(|ch| ch.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            node.set_character_positions(positions);
            node.set_character_widths(widths);
            node.set_text_direction(match direction {
                fission_ir::op::TextDirection::Rtl => AccessTextDirection::RightToLeft,
                _ => AccessTextDirection::LeftToRight,
            });
        }

        fn visual_rect(&self, node_id: WidgetId) -> Option<LayoutRect> {
            let mut rect = self.layout.get_node_rect(node_id)?;
            let mut current = self.ir.nodes.get(&node_id).and_then(|node| node.parent);
            while let Some(parent_id) = current {
                let parent = self.ir.nodes.get(&parent_id)?;
                match &parent.op {
                    Op::Layout(LayoutOp::Scroll { direction, .. }) => {
                        let offset = self.runtime.runtime_state.scroll.get_offset(parent_id);
                        match direction {
                            fission_ir::FlexDirection::Row => rect.origin.x -= offset,
                            fission_ir::FlexDirection::Column => rect.origin.y -= offset,
                        }
                        let viewport = self.layout.get_node_rect(parent_id)?;
                        rect = intersect_layout_rect(rect, viewport)?;
                    }
                    Op::Layout(LayoutOp::Transform { transform }) => {
                        let parent_rect = self.layout.get_node_rect(parent_id)?;
                        rect = transform_layout_rect(rect, parent_rect.origin, *transform);
                    }
                    Op::Layout(LayoutOp::InteractiveViewport { clip, .. }) => {
                        let viewport = self.layout.get_node_rect(parent_id)?;
                        let transform = self
                            .runtime
                            .runtime_state
                            .viewport
                            .transform(parent_id)
                            .unwrap_or_default();
                        let matrix = [
                            transform.scale,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                            transform.scale,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                            1.0,
                            0.0,
                            viewport.x() - viewport.x() * transform.scale
                                + transform.translation[0],
                            viewport.y() - viewport.y() * transform.scale
                                + transform.translation[1],
                            0.0,
                            1.0,
                        ];
                        rect = transform_layout_rect(rect, LayoutPoint::ZERO, matrix);
                        if !matches!(clip, fission_ir::ViewportClip::None) {
                            rect = intersect_layout_rect(rect, viewport)?;
                        }
                    }
                    _ => {}
                }
                current = parent.parent;
            }
            Some(rect)
        }
    }

    fn include_semantics(semantics: &Semantics) -> bool {
        semantics.role != Role::Generic
            || semantics.label.is_some()
            || semantics.identifier.is_some()
            || semantics.value.is_some()
            || semantics.focusable
            || semantics.checked.is_some()
            || semantics.current_value.is_some()
            || semantics.scrollable_x
            || semantics.scrollable_y
            || !semantics.actions.entries.is_empty()
    }

    fn access_role_for(semantics: &Semantics) -> AccessRole {
        match semantics.role {
            Role::Button => AccessRole::Button,
            Role::Link => AccessRole::Link,
            Role::MenuItem => AccessRole::MenuItem,
            Role::Text => AccessRole::Label,
            Role::TextInput if semantics.masked => AccessRole::PasswordInput,
            Role::TextInput if semantics.multiline => AccessRole::MultilineTextInput,
            Role::TextInput => match semantics.text_input_type {
                TextInputType::EmailAddress => AccessRole::EmailInput,
                TextInputType::Number => AccessRole::NumberInput,
                TextInputType::Phone => AccessRole::PhoneNumberInput,
                TextInputType::Url => AccessRole::UrlInput,
                TextInputType::Multiline => AccessRole::MultilineTextInput,
                _ => AccessRole::TextInput,
            },
            Role::Image => AccessRole::Image,
            Role::Checkbox => AccessRole::CheckBox,
            Role::Radio => AccessRole::RadioButton,
            Role::Switch => AccessRole::Switch,
            Role::Dialog => AccessRole::Dialog,
            Role::Slider => AccessRole::Slider,
            Role::Input => AccessRole::TextInput,
            Role::List => AccessRole::List,
            Role::ListItem => AccessRole::ListItem,
            Role::Generic => AccessRole::GenericContainer,
        }
    }

    fn intersect_layout_rect(first: LayoutRect, second: LayoutRect) -> Option<LayoutRect> {
        let left = first.x().max(second.x());
        let top = first.y().max(second.y());
        let right = first.right().min(second.right());
        let bottom = first.bottom().min(second.bottom());
        (right > left && bottom > top)
            .then(|| LayoutRect::new(left, top, right - left, bottom - top))
    }

    fn transform_layout_rect(
        rect: LayoutRect,
        origin: LayoutPoint,
        matrix: [f32; 16],
    ) -> LayoutRect {
        let corners = [
            (rect.x(), rect.y()),
            (rect.right(), rect.y()),
            (rect.right(), rect.bottom()),
            (rect.x(), rect.bottom()),
        ];
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for (x, y) in corners {
            let local_x = x - origin.x;
            let local_y = y - origin.y;
            let transformed_x = matrix[0] * local_x + matrix[4] * local_y + matrix[12] + origin.x;
            let transformed_y = matrix[1] * local_x + matrix[5] * local_y + matrix[13] + origin.y;
            min_x = min_x.min(transformed_x);
            min_y = min_y.min(transformed_y);
            max_x = max_x.max(transformed_x);
            max_y = max_y.max(transformed_y);
        }
        LayoutRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    fn accesskit_rect(rect: LayoutRect, scale_factor: f64) -> Rect {
        let x0 = rect.x() as f64 * scale_factor;
        let y0 = rect.y() as f64 * scale_factor;
        Rect::new(
            x0,
            y0,
            x0 + rect.width() as f64 * scale_factor,
            y0 + rect.height() as f64 * scale_factor,
        )
    }

    fn resolved_paragraph_in_subtree<'a>(
        ir: &'a CoreIR,
        layout: &'a LayoutSnapshot,
        node_id: WidgetId,
        expected_text_len: usize,
    ) -> Option<(&'a ResolvedParagraphLayout, fission_ir::op::TextDirection)> {
        let node = ir.nodes.get(&node_id)?;
        let paragraph_style = match &node.op {
            Op::Paint(PaintOp::DrawText {
                text,
                paragraph_style,
                ..
            }) if text.len() == expected_text_len => paragraph_style.as_ref(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                paragraph_style,
                ..
            }) if runs.iter().map(|run| run.text.len()).sum::<usize>() == expected_text_len => {
                paragraph_style.as_ref()
            }
            _ => None,
        };
        if matches!(
            &node.op,
            Op::Paint(PaintOp::DrawText { .. } | PaintOp::DrawRichText { .. })
        ) {
            if let Some(paragraph) = layout.get_resolved_paragraph(node_id) {
                let mut direction = paragraph_style
                    .map(|style| style.text_direction)
                    .unwrap_or_default();
                if direction == fission_ir::op::TextDirection::Auto {
                    direction = if paragraph
                        .clusters
                        .first()
                        .is_some_and(|cluster| cluster.is_rtl)
                    {
                        fission_ir::op::TextDirection::Rtl
                    } else {
                        fission_ir::op::TextDirection::Ltr
                    };
                }
                return Some((paragraph, direction));
            }
        }
        node.children
            .iter()
            .find_map(|child| resolved_paragraph_in_subtree(ir, layout, *child, expected_text_len))
    }

    fn paragraph_character_geometry(
        paragraph: &ResolvedParagraphLayout,
        text: &str,
        declared_direction: fission_ir::op::TextDirection,
    ) -> (Vec<f32>, Vec<f32>) {
        let base_rtl = match declared_direction {
            fission_ir::op::TextDirection::Rtl => true,
            fission_ir::op::TextDirection::Ltr => false,
            fission_ir::op::TextDirection::Auto => paragraph
                .clusters
                .iter()
                .find(|cluster| cluster.end_index > cluster.start_index)
                .is_some_and(|cluster| cluster.is_rtl),
        };
        let mut positions = Vec::with_capacity(text.chars().count());
        let mut widths = Vec::with_capacity(text.chars().count());
        for (byte_index, _) in text.char_indices() {
            let next = text[byte_index..]
                .char_indices()
                .nth(1)
                .map(|(offset, _)| byte_index + offset)
                .unwrap_or(text.len());
            if let Some(cluster) = paragraph
                .clusters
                .iter()
                .find(|cluster| cluster.start_index < next && cluster.end_index > byte_index)
            {
                let indices = text[cluster.start_index..cluster.end_index]
                    .char_indices()
                    .map(|(offset, _)| cluster.start_index + offset)
                    .collect::<Vec<_>>();
                let count = indices.len().max(1) as f32;
                let ordinal = indices
                    .iter()
                    .position(|index| *index == byte_index)
                    .unwrap_or(0) as f32;
                let width = cluster.rect.width() / count;
                let left = if cluster.is_rtl {
                    cluster.rect.right() - (ordinal + 1.0) * width
                } else {
                    cluster.rect.x() + ordinal * width
                };
                positions.push(if base_rtl {
                    paragraph.size.width - left - width
                } else {
                    left
                });
                widths.push(width);
            } else {
                let x = paragraph
                    .caret(byte_index, false)
                    .map(|caret| caret.position.x)
                    .unwrap_or(0.0);
                positions.push(if base_rtl {
                    paragraph.size.width - x
                } else {
                    x
                });
                widths.push(0.0);
            }
        }
        (positions, widths)
    }

    fn semantics_for(ir: &CoreIR, id: WidgetId) -> Option<&Semantics> {
        ir.nodes.get(&id).and_then(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
    }

    fn semantic_value(
        runtime: &Runtime,
        node_id: WidgetId,
        semantics: &Semantics,
    ) -> Option<String> {
        if semantics.role == Role::TextInput {
            if semantics.masked {
                return None;
            }
            semantics.value.clone().or_else(|| {
                runtime
                    .runtime_state
                    .text_edit
                    .get(node_id)
                    .map(|state| state.committed_text())
            })
        } else {
            semantics.value.clone()
        }
    }

    fn collect_descendant_text(ir: &CoreIR, node_id: WidgetId) -> Option<String> {
        let mut out = String::new();
        collect_descendant_text_inner(ir, node_id, &mut out);
        let trimmed = out.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    fn collect_descendant_text_inner(ir: &CoreIR, node_id: WidgetId, out: &mut String) {
        let Some(node) = ir.nodes.get(&node_id) else {
            return;
        };
        match &node.op {
            Op::Paint(PaintOp::DrawText { text, .. }) => {
                if !text.is_empty() {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(text);
                }
            }
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                if !text.is_empty() {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(&text);
                }
            }
            _ => {
                for child in &node.children {
                    collect_descendant_text_inner(ir, *child, out);
                }
            }
        }
    }

    fn set_focus(runtime: &mut Runtime, ir: &CoreIR, focus: Option<WidgetId>) -> bool {
        runtime
            .set_focused_widget(ir, focus, fission_core::TextEditSource::Accessibility)
            .unwrap_or(false)
    }

    fn dispatch_semantics_action(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        trigger: ActionTrigger,
        input: ActionInput,
    ) -> bool {
        let Some(entry) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == trigger)
        else {
            return false;
        };
        let envelope = ActionEnvelope {
            id: ActionId::from_u128(entry.action_id),
            payload: entry.payload_data.clone().unwrap_or_default(),
        };
        let input = scoped_semantics_input(ir, target, input);
        runtime
            .dispatch_with_input(envelope, target, &input)
            .is_ok()
    }

    fn scoped_semantics_input(ir: &CoreIR, target: WidgetId, input: ActionInput) -> ActionInput {
        let mut current = Some(target);
        while let Some(node_id) = current {
            let Some(node) = ir.nodes.get(&node_id) else {
                break;
            };
            if let Op::Semantics(semantics) = &node.op {
                if let Some(scope_id) = semantics.action_scope_id {
                    return ActionInput::scoped_raw(scope_id, target, input);
                }
            }
            current = node.parent;
        }
        input
    }

    fn value_action_data(data: &Option<ActionData>) -> Option<&str> {
        match data {
            Some(ActionData::Value(value)) => Some(value),
            _ => None,
        }
    }

    fn set_text_input_value(
        runtime: &mut Runtime,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        target: WidgetId,
        semantics: &Semantics,
        value: &str,
    ) -> bool {
        if !editable_text_input(semantics) {
            return false;
        }
        set_focus(runtime, ir, Some(target));
        runtime
            .apply_text_edit_command_to(
                ir,
                layout,
                target,
                fission_core::TextEditCommand::SetValue {
                    value: fission_core::TextEditingValue::from_text(value),
                    source: fission_core::TextEditSource::Accessibility,
                    phase: fission_core::TextValuePhase::Committed,
                },
            )
            .unwrap_or(false)
    }

    fn set_text_selection(
        runtime: &mut Runtime,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        target: WidgetId,
        semantics: &Semantics,
        selection: &TextSelection,
    ) -> bool {
        if semantics
            .selection_region
            .as_ref()
            .is_some_and(|region| !region.excluded)
            && !semantics.disabled
        {
            let changed = SelectionRegionController::new(target)
                .select_scalar_range(
                    &mut runtime.runtime_state,
                    ir,
                    selection.anchor.character_index,
                    selection.focus.character_index,
                    TextAffinity::Downstream,
                )
                .is_ok();
            if changed {
                set_focus(runtime, ir, Some(target));
            }
            return changed;
        }
        if semantics.role != Role::TextInput || semantics.disabled {
            return false;
        }
        set_focus(runtime, ir, Some(target));
        let value = runtime
            .runtime_state
            .text_edit
            .get(target)
            .map(|state| state.committed_text())
            .unwrap_or_else(|| semantics.value.clone().unwrap_or_default());
        let caret = char_to_byte(&value, selection.focus.character_index);
        let anchor = char_to_byte(&value, selection.anchor.character_index);
        runtime
            .apply_text_edit_command_to(
                ir,
                layout,
                target,
                fission_core::TextEditCommand::SetSelection {
                    selection: fission_core::TextSelection::new(
                        &value,
                        anchor,
                        caret,
                        fission_core::TextAffinity::Downstream,
                    )
                    .expect("accessibility offsets were converted from this value"),
                    source: fission_core::TextEditSource::Accessibility,
                },
            )
            .unwrap_or(false)
    }

    fn char_to_byte(value: &str, character_index: usize) -> usize {
        EditingTextPosition::from_scalar_offset(value, character_index)
            .unwrap_or_else(|_| EditingTextPosition::at_end(value))
            .utf8_offset()
    }

    fn byte_to_char(value: &str, byte_index: usize) -> usize {
        EditingTextPosition::from_utf8(value, byte_index)
            .or_else(|_| Ok(EditingTextPosition::floor(value, byte_index)))
            .and_then(|position| position.scalar_offset(value))
            .unwrap_or_else(|_| value.chars().count())
    }

    fn adjust_numeric_value(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        direction: f32,
    ) -> bool {
        let Some(current) = semantics.current_value else {
            return false;
        };
        let min = semantics.min_value.unwrap_or(f32::NEG_INFINITY);
        let max = semantics.max_value.unwrap_or(f32::INFINITY);
        let next = (current + direction).clamp(min, max);
        set_numeric_value(runtime, ir, target, semantics, next)
    }

    fn set_numeric_value(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        value: f32,
    ) -> bool {
        if semantics.current_value.is_none() {
            return false;
        }
        let Ok(payload) = serde_json::to_vec(&value) else {
            return false;
        };
        let Some(entry) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::Change)
        else {
            return false;
        };
        let input = scoped_semantics_input(ir, target, ActionInput::None);
        runtime
            .dispatch_with_input(
                ActionEnvelope {
                    id: ActionId::from_u128(entry.action_id),
                    payload,
                },
                target,
                &input,
            )
            .is_ok()
    }

    fn handle_scroll_action(
        runtime: &mut Runtime,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        target: WidgetId,
        action: Action,
        data: &Option<ActionData>,
    ) -> bool {
        let horizontal = matches!(action, Action::ScrollLeft | Action::ScrollRight);
        let Some(scroll_node) = find_scroll_node(ir, target, horizontal) else {
            return false;
        };
        let Some(geometry) = layout.get_node_geometry(scroll_node) else {
            return false;
        };
        let max_offset = if horizontal {
            (geometry.content_size.width - geometry.rect.width()).max(0.0)
        } else {
            (geometry.content_size.height - geometry.rect.height()).max(0.0)
        };
        let current = runtime.runtime_state.scroll.get_offset(scroll_node);
        let amount = match data {
            Some(ActionData::ScrollUnit(accesskit::ScrollUnit::Item)) => 40.0,
            _ => {
                if horizontal {
                    geometry.rect.width() * 0.8
                } else {
                    geometry.rect.height() * 0.8
                }
            }
        };
        let signed = match action {
            Action::ScrollUp | Action::ScrollLeft => -amount,
            _ => amount,
        };
        let next = (current + signed).clamp(0.0, max_offset);
        if (next - current).abs() <= 0.001 {
            return false;
        }
        runtime.runtime_state.scroll.set_offset(scroll_node, next);
        true
    }

    fn scroll_offset_and_max(
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        runtime: &Runtime,
        target: WidgetId,
        horizontal: bool,
    ) -> Option<(f32, f32)> {
        let scroll_node = find_scroll_node(ir, target, horizontal)?;
        let geometry = layout.get_node_geometry(scroll_node)?;
        let max = if horizontal {
            (geometry.content_size.width - geometry.rect.width()).max(0.0)
        } else {
            (geometry.content_size.height - geometry.rect.height()).max(0.0)
        };
        Some((runtime.runtime_state.scroll.get_offset(scroll_node), max))
    }

    fn find_scroll_node(ir: &CoreIR, target: WidgetId, horizontal: bool) -> Option<WidgetId> {
        let target_direction = if horizontal {
            fission_ir::FlexDirection::Row
        } else {
            fission_ir::FlexDirection::Column
        };
        let mut stack = vec![target];
        while let Some(id) = stack.pop() {
            let Some(node) = ir.nodes.get(&id) else {
                continue;
            };
            if let Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) = &node.op {
                if *direction == target_direction {
                    return Some(id);
                }
            }
            stack.extend(node.children.iter().rev().copied());
        }
        None
    }

    #[cfg(test)]
    mod tests {
        include!("accessibility_tests.rs");
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use fission_core::Runtime;
    use fission_ir::CoreIR;
    use fission_layout::LayoutSnapshot;
    use fission_test_driver::TestEvent;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    pub struct AccessibilityBridge;

    impl AccessibilityBridge {
        pub fn new(_proxy: EventLoopProxy<TestEvent>) -> Self {
            Self
        }

        pub fn ensure_adapter(&mut self, _event_loop: &ActiveEventLoop, _window: &Window) {}

        pub fn process_window_event(&mut self, _window: &Window, _event: &WindowEvent) {}

        pub fn update_tree(
            &mut self,
            _ir: &CoreIR,
            _layout: &LayoutSnapshot,
            _runtime: &Runtime,
            _scale_factor: f64,
        ) {
        }

        pub fn drain_events(
            &mut self,
            _runtime: &mut Runtime,
            _ir: Option<&CoreIR>,
            _layout: Option<&LayoutSnapshot>,
        ) -> bool {
            false
        }
    }

    pub fn window_must_start_hidden() -> bool {
        false
    }
}

pub use imp::{window_must_start_hidden, AccessibilityBridge};
