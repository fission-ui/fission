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

        fn handle_action_request(
            &mut self,
            request: ActionRequest,
            runtime: &mut Runtime,
            ir: &CoreIR,
            layout: &LayoutSnapshot,
        ) -> bool {
            self.active = true;
            let node_map = self.shared.latest_node_map.lock().ok();
            let Some(target) = node_map
                .as_ref()
                .and_then(|map| map.get(&request.target_node).copied())
            else {
                return false;
            };
            let Some(semantics) = semantics_for(ir, target) else {
                return false;
            };

            match request.action {
                Action::Click => dispatch_semantics_action(
                    runtime,
                    target,
                    semantics,
                    ActionTrigger::Default,
                    ActionInput::None,
                ),
                Action::Focus => set_focus(runtime, ir, Some(target)),
                Action::Blur => set_focus(runtime, ir, None),
                Action::ReplaceSelectedText => {
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
                    Some(ActionData::NumericValue(value)) => set_numeric_value(
                        runtime,
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
                Action::ScrollDown
                | Action::ScrollUp
                | Action::ScrollLeft
                | Action::ScrollRight => {
                    handle_scroll_action(runtime, ir, layout, target, request.action, &request.data)
                }
                Action::Increment => adjust_numeric_value(runtime, target, semantics, 1.0),
                Action::Decrement => adjust_numeric_value(runtime, target, semantics, -1.0),
                _ => false,
            }
        }
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
                    node.add_action(Action::ReplaceSelectedText);
                    node.add_action(Action::SetValue);
                    node.add_action(Action::SetTextSelection);
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
        let input = scoped_semantics_input(target, semantics, input);
        runtime
            .dispatch_with_input(envelope, target, &input)
            .is_ok()
    }

    fn scoped_semantics_input(
        target: WidgetId,
        semantics: &Semantics,
        input: ActionInput,
    ) -> ActionInput {
        if let Some(scope_id) = semantics.action_scope_id {
            ActionInput::scoped_raw(scope_id, target, input)
        } else {
            input
        }
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
        if semantics.role != Role::TextInput || semantics.disabled || semantics.read_only {
            return false;
        }
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
        let mut changed = dispatch_text_change(runtime, target, semantics, value.to_string());
        changed |= dispatch_cursor_change(runtime, target, semantics, value.len(), value.len());
        changed
    }

    fn set_text_selection(
        runtime: &mut Runtime,
        ir: &CoreIR,
        target: WidgetId,
        semantics: &Semantics,
        selection: &TextSelection,
    ) -> bool {
        if semantics.role != Role::TextInput {
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
        dispatch_cursor_change(runtime, target, semantics, caret, anchor)
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
        target: WidgetId,
        semantics: &Semantics,
        new_text: String,
    ) -> bool {
        let Some(entry) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::Change)
        else {
            return false;
        };
        let Ok(payload) = serde_json::to_vec(&new_text) else {
            return false;
        };
        let input = scoped_semantics_input(target, semantics, ActionInput::None);
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

    fn dispatch_cursor_change(
        runtime: &mut Runtime,
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
        let input = scoped_semantics_input(target, semantics, ActionInput::None);
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
        set_numeric_value(runtime, target, semantics, next)
    }

    fn set_numeric_value(
        runtime: &mut Runtime,
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
        let input = scoped_semantics_input(target, semantics, ActionInput::None);
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
        use fission_ir::{ActionEntry, ActionSet, CoreIR, CoreNode, Op};
        use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutSize};

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
    }
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
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
