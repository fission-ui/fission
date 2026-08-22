#[cfg(any(test, target_arch = "wasm32"))]
mod model;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod imp {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::{Arc, Mutex};

    use accesskit::{
        Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler,
        Node, NodeId, Rect, Role as AccessRole, TextPosition, TextSelection, Toggled, Tree, TreeId,
        TreeUpdate,
    };
    use accesskit_winit::Adapter;
    use fission_core::event::ImeEvent;
    use fission_core::input::prepare_scoped_text_input_change;
    use fission_core::{ActionEnvelope, ActionId, ActionInput, InputEvent, Runtime};
    use fission_ir::semantics::{ActionTrigger, Role, TextInputType};
    use fission_ir::{CoreIR, Op, PaintOp, Semantics, WidgetId};
    use fission_layout::{LayoutRect, LayoutSnapshot};
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

        pub fn resume(&mut self) {}

        pub fn suspend(&mut self) {}

        pub fn shutdown(&mut self) {}

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
                runtime
                    .handle_input(
                        InputEvent::Ime(ImeEvent::Commit {
                            text: text.to_string(),
                        }),
                        ir,
                        layout,
                    )
                    .is_ok()
            }
            Action::SetValue => match &request.data {
                Some(ActionData::Value(value)) => {
                    set_text_input_value(runtime, ir, target, semantics, value)
                }
                Some(ActionData::NumericValue(value)) if semantics.role == Role::TextInput => {
                    set_text_input_value(runtime, ir, target, semantics, &value.to_string())
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
                set_text_selection(runtime, ir, target, semantics, selection)
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
                    let child_ids = core_node
                        .children
                        .iter()
                        .flat_map(|child| self.collect_subtree(*child, true))
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
                    if let Some(rect) = self.layout.get_node_rect(node_id) {
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
                    node.set_value(text);
                    if let Some(rect) = self.layout.get_node_rect(node_id) {
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
            if let Some(rect) = self.layout.get_node_rect(node_id) {
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
                    if let Some(text) = label.or(value.clone()) {
                        node.set_value(text);
                    }
                    node.set_read_only();
                }
                Role::TextInput => {
                    if let Some(label) = label {
                        node.set_label(label);
                    }
                    if let Some(value) = value {
                        node.set_value(value.clone());
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
        let old_focus = runtime.runtime_state.interaction.focused;
        if old_focus == focus {
            return false;
        }
        if let Some(old_id) = old_focus {
            if let Some(state) = runtime.runtime_state.text_edit.states.get_mut(&old_id) {
                state.pending_model_sync = false;
                state.clear_preedit();
            }
            if let Some(old_semantics) = semantics_for(ir, old_id) {
                let _ = dispatch_semantics_action(
                    runtime,
                    ir,
                    old_id,
                    old_semantics,
                    ActionTrigger::Blur,
                    ActionInput::None,
                );
            }
        }
        runtime.runtime_state.interaction.set_focused(focus);
        if let Some(ime_handler) = &runtime.ime_handler {
            let allow_ime = focus
                .and_then(|id| semantics_for(ir, id))
                .map(|semantics| {
                    semantics.role == Role::TextInput && !semantics.disabled && !semantics.read_only
                })
                .unwrap_or(false);
            ime_handler.set_ime_allowed(allow_ime);
        }
        if let Some(new_id) = focus {
            if let Some(new_semantics) = semantics_for(ir, new_id) {
                let _ = dispatch_semantics_action(
                    runtime,
                    ir,
                    new_id,
                    new_semantics,
                    ActionTrigger::Focus,
                    ActionInput::None,
                );
            }
        }
        true
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
        target: WidgetId,
        semantics: &Semantics,
        value: &str,
    ) -> bool {
        if !editable_text_input(semantics) {
            return false;
        }
        let previous_text_state = runtime.runtime_state.text_edit.get(target).cloned();
        set_focus(runtime, ir, Some(target));
        runtime.runtime_state.text_edit.sync_from_runtime(
            target,
            semantics.value.as_deref().unwrap_or_default(),
            None,
            None,
        );
        {
            let state = runtime.runtime_state.text_edit.get_mut_or_default(target);
            let old_len = state.buffer.len_bytes();
            state.buffer.replace(0..old_len, value);
            state.caret = value.len();
            state.anchor = value.len();
            state.pending_model_sync = true;
            state.clear_preedit();
        }
        if !dispatch_text_change(
            runtime,
            ir,
            target,
            semantics,
            value.to_string(),
            value.len(),
            value.len(),
        ) {
            if let Some(previous) = previous_text_state {
                runtime
                    .runtime_state
                    .text_edit
                    .states
                    .insert(target, previous);
            } else {
                runtime.runtime_state.text_edit.states.remove(&target);
            }
            return false;
        }

        let has_cursor_action = semantics
            .actions
            .entries
            .iter()
            .any(|entry| entry.trigger == ActionTrigger::CursorChange);
        let cursor_changed =
            dispatch_cursor_change(runtime, ir, target, semantics, value.len(), value.len());
        !has_cursor_action || cursor_changed
    }

    fn set_text_selection(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        selection: &TextSelection,
    ) -> bool {
        if semantics.role != Role::TextInput || semantics.disabled {
            return false;
        }
        set_focus(runtime, ir, Some(target));
        runtime.runtime_state.text_edit.sync_from_runtime(
            target,
            semantics.value.as_deref().unwrap_or_default(),
            None,
            None,
        );
        let value = runtime
            .runtime_state
            .text_edit
            .get(target)
            .map(|state| state.committed_text())
            .unwrap_or_default();
        let caret = char_to_byte(&value, selection.focus.character_index);
        let anchor = char_to_byte(&value, selection.anchor.character_index);
        runtime
            .runtime_state
            .text_edit
            .set_caret(target, caret, Some(anchor));
        dispatch_cursor_change(runtime, ir, target, semantics, caret, anchor)
    }

    fn char_to_byte(value: &str, character_index: usize) -> usize {
        value
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(value.len()))
            .nth(character_index)
            .unwrap_or(value.len())
    }

    fn byte_to_char(value: &str, byte_index: usize) -> usize {
        let mut clamped = byte_index.min(value.len());
        while clamped > 0 && !value.is_char_boundary(clamped) {
            clamped -= 1;
        }
        value[..clamped].chars().count()
    }

    fn dispatch_text_change(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        new_text: String,
        new_caret: usize,
        new_anchor: usize,
    ) -> bool {
        let Some((envelope, input)) = prepare_scoped_text_input_change(
            ir, semantics, target, new_text, new_caret, new_anchor,
        ) else {
            crate::log_input_dispatch_failure(
                "accessibility_text_input_missing_action",
                Some(target),
            );
            return false;
        };
        runtime
            .dispatch_with_input(envelope, target, &input)
            .is_ok()
    }

    fn dispatch_cursor_change(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        caret: usize,
        anchor: usize,
    ) -> bool {
        let Some(entry) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::CursorChange)
        else {
            return false;
        };
        let cursor_changed = fission_core::action::CursorChanged { caret, anchor };
        let Ok(payload) = serde_json::to_vec(&cursor_changed) else {
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
        use super::*;
        use fission_core::action::CursorChanged;
        use fission_core::{
            Action as FissionAction, ActionId as FissionActionId, ActionRegistry, GlobalState,
            ReducerContext, UpdateTextInput,
        };
        use fission_ir::{ActionEntry, ActionSet, CoreIR, CoreNode, Op};
        use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutSize};
        use serde::{Deserialize, Serialize};
        use std::collections::BTreeMap;

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
            cursors: Vec<(CursorChanged, Option<u128>, Option<WidgetId>)>,
        }

        impl GlobalState for TextDispatchState {}

        fn record_contextual_edit(
            state: &mut TextDispatchState,
            action: UpdateField,
            ctx: &mut ReducerContext<TextDispatchState>,
        ) {
            let change = ctx
                .input
                .text_change()
                .expect("contextual edit input")
                .clone();
            state.contextual.insert(
                (ctx.input.action_scope_id().unwrap_or_default(), action.0),
                RecordedContextualEdit {
                    change,
                    scoped_target: ctx.input.scoped_target(),
                },
            );
        }

        fn record_cursor(
            state: &mut TextDispatchState,
            action: CursorChanged,
            ctx: &mut ReducerContext<TextDispatchState>,
        ) {
            state.cursors.push((
                action,
                ctx.input.action_scope_id(),
                ctx.input.scoped_target(),
            ));
        }

        fn text_dispatch_runtime() -> Runtime {
            let mut runtime = Runtime::default();
            runtime
                .add_app_state(Box::new(TextDispatchState::default()))
                .expect("register text dispatch test state");
            let mut registry = ActionRegistry::<TextDispatchState>::new();
            registry.register(
                record_contextual_edit
                    as fn(
                        &mut TextDispatchState,
                        UpdateField,
                        &mut ReducerContext<TextDispatchState>,
                    ),
            );
            registry.register(
                record_cursor
                    as fn(
                        &mut TextDispatchState,
                        CursorChanged,
                        &mut ReducerContext<TextDispatchState>,
                    ),
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

        #[test]
        fn text_input_value_prefers_lowered_semantics_over_retained_runtime_buffer() {
            let input = WidgetId::from_u128(20);
            let mut runtime = Runtime::default();
            runtime.runtime_state.text_edit.sync_from_runtime(
                input,
                "Stale retained buffer",
                None,
                None,
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
            assert_eq!(
                first_edit.change,
                UpdateTextInput {
                    node_id: first,
                    new_text: "greenmail".into(),
                    new_caret: "greenmail".len(),
                    new_anchor: "greenmail".len(),
                }
            );
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

            assert_eq!(native_edit, accessibility_edit);
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
                        action_id: CursorChanged::static_id().as_u128(),
                        payload_data: None,
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
                target,
                &semantics,
                &selection,
            ));

            let state = runtime
                .get_app_state::<TextDispatchState>()
                .expect("text dispatch state");
            assert_eq!(
                state.cursors,
                [(
                    CursorChanged {
                        caret: 7,
                        anchor: 1,
                    },
                    Some(scope_id),
                    Some(target),
                )]
            );
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
    }
}

#[cfg(target_os = "android")]
#[path = "accessibility/android.rs"]
mod imp;

#[cfg(all(target_arch = "wasm32", not(target_os = "android")))]
#[path = "accessibility/web.rs"]
mod imp;

pub use imp::{window_must_start_hidden, AccessibilityBridge};
