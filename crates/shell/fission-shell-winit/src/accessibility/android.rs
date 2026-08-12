use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fission_core::event::ImeEvent;
use fission_core::{ActionEnvelope, ActionId, ActionInput, InputEvent, Runtime};
use fission_ir::semantics::{ActionTrigger, Role, TextInputType};
use fission_ir::{CoreIR, Op, PaintOp, Semantics, WidgetId};
use fission_layout::{LayoutRect, LayoutSnapshot};
use fission_test_driver::TestEvent;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

use crate::android_host::{AndroidHostBridge, AndroidHostEvent, AndroidSemanticsNode};

const FLAG_ENABLED: i32 = 1 << 0;
const FLAG_FOCUSABLE: i32 = 1 << 1;
const FLAG_FOCUSED: i32 = 1 << 2;
const FLAG_CHECKABLE: i32 = 1 << 3;
const FLAG_CHECKED: i32 = 1 << 4;
const FLAG_EDITABLE: i32 = 1 << 5;
const FLAG_PASSWORD: i32 = 1 << 6;
const FLAG_MULTILINE: i32 = 1 << 7;
const FLAG_SCROLL_X: i32 = 1 << 8;
const FLAG_SCROLL_Y: i32 = 1 << 9;
const FLAG_READ_ONLY: i32 = 1 << 10;

const ACTION_CLICK: i32 = 1 << 0;
const ACTION_FOCUS: i32 = 1 << 1;
const ACTION_SET_TEXT: i32 = 1 << 2;
const ACTION_SET_SELECTION: i32 = 1 << 3;
const ACTION_SCROLL_X: i32 = 1 << 4;
const ACTION_SCROLL_Y: i32 = 1 << 5;
const ACTION_INCREMENT: i32 = 1 << 6;
const ACTION_DECREMENT: i32 = 1 << 7;

pub struct AccessibilityBridge {
    host: Arc<AndroidHostBridge>,
    node_map: HashMap<i32, WidgetId>,
    last_error: Option<String>,
}

impl AccessibilityBridge {
    pub fn new(_proxy: EventLoopProxy<TestEvent>, host: Arc<AndroidHostBridge>) -> Self {
        Self {
            host,
            node_map: HashMap::new(),
            last_error: None,
        }
    }

    pub fn ensure_adapter(&mut self, _event_loop: &ActiveEventLoop, _window: &Window) {
        self.resume();
    }

    pub fn process_window_event(&mut self, _window: &Window, _event: &WindowEvent) {}

    pub fn update_tree(
        &mut self,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        runtime: &Runtime,
        scale_factor: f64,
    ) {
        let built = SemanticsBuilder::new(ir, layout, runtime, scale_factor).build();
        match self.host.update_semantics(&built.nodes, built.focused_id) {
            Ok(()) => {
                self.node_map = built.node_map;
                self.report(Ok(()));
            }
            Err(error) => self.report(Err(error)),
        }
    }

    pub fn drain_events(
        &mut self,
        runtime: &mut Runtime,
        ir: Option<&CoreIR>,
        layout: Option<&LayoutSnapshot>,
    ) -> bool {
        let events = match self.host.drain_events() {
            Ok(events) => {
                self.last_error = None;
                events
            }
            Err(error) => {
                self.report(Err(error));
                return false;
            }
        };
        let (Some(ir), Some(layout)) = (ir, layout) else {
            for event in events {
                if let AndroidHostEvent::HostError(error) = event {
                    self.report(Err(error));
                }
            }
            return false;
        };
        events.into_iter().fold(false, |changed, event| {
            self.handle_event(event, runtime, ir, layout) || changed
        })
    }

    pub fn resume(&mut self) {
        let result = self.host.set_active(true);
        self.report(result);
    }

    pub fn suspend(&mut self) {
        let result = self.host.set_active(false);
        self.report(result);
    }

    pub fn shutdown(&mut self) {
        self.suspend();
    }

    fn handle_event(
        &mut self,
        event: AndroidHostEvent,
        runtime: &mut Runtime,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> bool {
        let event = match event {
            AndroidHostEvent::HostError(error) => {
                self.report(Err(error));
                return false;
            }
            event => event,
        };
        let target = |id| self.node_map.get(&id).copied();
        match event {
            AndroidHostEvent::HostError(_) => unreachable!("host errors return above"),
            AndroidHostEvent::Click(id) => target(id)
                .and_then(|id| semantics_for(ir, id).map(|semantics| (id, semantics)))
                .map(|(id, semantics)| {
                    dispatch_semantics_action(
                        runtime,
                        id,
                        semantics,
                        ActionTrigger::Default,
                        ActionInput::None,
                        None,
                    )
                })
                .unwrap_or(false),
            AndroidHostEvent::Focus(id) => target(id)
                .map(|id| set_focus(runtime, ir, Some(id)))
                .unwrap_or(false),
            AndroidHostEvent::Blur(id) => target(id)
                .filter(|id| runtime.runtime_state.interaction.focused == Some(*id))
                .map(|_| set_focus(runtime, ir, None))
                .unwrap_or(false),
            AndroidHostEvent::SetText { id, value } => target(id)
                .and_then(|id| semantics_for(ir, id).map(|semantics| (id, semantics)))
                .map(|(id, semantics)| set_text_input_value(runtime, ir, id, semantics, &value))
                .unwrap_or(false),
            AndroidHostEvent::SetSelection {
                id,
                start_utf16,
                end_utf16,
            } => target(id)
                .and_then(|id| semantics_for(ir, id).map(|semantics| (id, semantics)))
                .map(|(id, semantics)| {
                    set_text_selection_utf16(runtime, ir, id, semantics, start_utf16, end_utf16)
                })
                .unwrap_or(false),
            AndroidHostEvent::Scroll { id, direction } => target(id)
                .and_then(|id| semantics_for(ir, id).map(|semantics| (id, semantics)))
                .map(|(id, semantics)| handle_scroll(runtime, ir, layout, id, semantics, direction))
                .unwrap_or(false),
            AndroidHostEvent::Increment { id, direction } => target(id)
                .and_then(|id| semantics_for(ir, id).map(|semantics| (id, semantics)))
                .map(|(id, semantics)| {
                    adjust_numeric_value(runtime, id, semantics, direction.signum() as f32)
                })
                .unwrap_or(false),
            AndroidHostEvent::SetNumeric { id, value } => target(id)
                .and_then(|id| semantics_for(ir, id).map(|semantics| (id, semantics)))
                .map(|(id, semantics)| set_numeric_value(runtime, id, semantics, value))
                .unwrap_or(false),
            AndroidHostEvent::ImeCommit(text) => runtime
                .handle_input(InputEvent::Ime(ImeEvent::Commit { text }), ir, layout)
                .is_ok(),
            AndroidHostEvent::ImePreedit { text, cursor_utf16 } => {
                let cursor = cursor_utf16
                    .map(|(start, end)| (utf16_to_byte(&text, start), utf16_to_byte(&text, end)));
                runtime
                    .handle_input(
                        InputEvent::Ime(ImeEvent::Preedit { text, cursor }),
                        ir,
                        layout,
                    )
                    .is_ok()
            }
            AndroidHostEvent::ImeCancel => runtime
                .handle_input(InputEvent::Ime(ImeEvent::Cancel), ir, layout)
                .is_ok(),
            AndroidHostEvent::ImeReplace {
                value,
                selection_utf16,
            } => focused_semantics(runtime, ir)
                .map(|(id, semantics)| {
                    replace_text_input_value(
                        runtime,
                        ir,
                        id,
                        semantics,
                        &value,
                        Some(selection_utf16),
                    )
                })
                .unwrap_or(false),
            AndroidHostEvent::ImeSelection {
                start_utf16,
                end_utf16,
            } => focused_semantics(runtime, ir)
                .map(|(id, semantics)| {
                    set_text_selection_utf16(runtime, ir, id, semantics, start_utf16, end_utf16)
                })
                .unwrap_or(false),
            AndroidHostEvent::ImeAction => focused_semantics(runtime, ir)
                .map(|(id, semantics)| {
                    let value = current_text_value(runtime, id, semantics);
                    let payload = serde_json::to_vec(&value).ok();
                    let mut changed = dispatch_semantics_action(
                        runtime,
                        id,
                        semantics,
                        ActionTrigger::EditingComplete,
                        ActionInput::None,
                        payload.clone(),
                    );
                    changed |= dispatch_semantics_action(
                        runtime,
                        id,
                        semantics,
                        ActionTrigger::Submit,
                        ActionInput::None,
                        payload,
                    );
                    changed
                })
                .unwrap_or(false),
        }
    }

    fn report(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.last_error = None,
            Err(error) if self.last_error.as_deref() != Some(&error) => {
                eprintln!("fission-shell-winit: Android host error: {error}");
                self.last_error = Some(error);
            }
            Err(_) => {}
        }
    }
}

pub fn window_must_start_hidden() -> bool {
    false
}

struct BuiltSemantics {
    nodes: Vec<AndroidSemanticsNode>,
    node_map: HashMap<i32, WidgetId>,
    focused_id: i32,
}

struct SemanticsBuilder<'a> {
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    runtime: &'a Runtime,
    scale_factor: f64,
    nodes: Vec<AndroidSemanticsNode>,
    used_ids: HashSet<i32>,
    widget_to_node: HashMap<WidgetId, i32>,
    node_map: HashMap<i32, WidgetId>,
}

impl<'a> SemanticsBuilder<'a> {
    fn new(
        ir: &'a CoreIR,
        layout: &'a LayoutSnapshot,
        runtime: &'a Runtime,
        scale_factor: f64,
    ) -> Self {
        Self {
            ir,
            layout,
            runtime,
            scale_factor,
            nodes: Vec::new(),
            used_ids: HashSet::new(),
            widget_to_node: HashMap::new(),
            node_map: HashMap::new(),
        }
    }

    fn build(mut self) -> BuiltSemantics {
        if let Some(root) = self.ir.root {
            self.collect_subtree(root, 0, false);
        }
        let focused_id = self
            .runtime
            .runtime_state
            .interaction
            .focused
            .and_then(|id| self.widget_to_node.get(&id).copied())
            .unwrap_or(0);
        BuiltSemantics {
            nodes: self.nodes,
            node_map: self.node_map,
            focused_id,
        }
    }

    fn collect_subtree(&mut self, node_id: WidgetId, parent_id: i32, inside_semantics: bool) {
        let Some(core_node) = self.ir.nodes.get(&node_id) else {
            return;
        };
        match &core_node.op {
            Op::Semantics(semantics) if include_semantics(semantics) => {
                let id = self.node_id_for(node_id);
                self.nodes
                    .push(self.node_for_semantics(id, parent_id, node_id, semantics));
                for child in &core_node.children {
                    self.collect_subtree(*child, id, true);
                }
            }
            Op::Paint(PaintOp::DrawText { text, .. }) if !text.is_empty() => {
                self.push_text(node_id, parent_id, text.clone(), inside_semantics);
            }
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                if !text.is_empty() {
                    self.push_text(node_id, parent_id, text, inside_semantics);
                }
            }
            _ => {
                for child in &core_node.children {
                    self.collect_subtree(*child, parent_id, inside_semantics);
                }
            }
        }
    }

    fn push_text(&mut self, node_id: WidgetId, parent_id: i32, text: String, read_only: bool) {
        let id = self.node_id_for(node_id);
        self.nodes.push(AndroidSemanticsNode {
            id,
            parent_id,
            role: 4,
            flags: FLAG_ENABLED | if read_only { FLAG_READ_ONLY } else { 0 },
            actions: 0,
            bounds: self.bounds(node_id),
            label: None,
            value: Some(text),
            selection_utf16: [-1, -1],
            numeric: [f32::NAN; 3],
        });
    }

    fn node_for_semantics(
        &self,
        id: i32,
        parent_id: i32,
        node_id: WidgetId,
        semantics: &Semantics,
    ) -> AndroidSemanticsNode {
        let value = current_text_value(self.runtime, node_id, semantics);
        let label = semantics
            .label
            .clone()
            .or_else(|| collect_descendant_text(self.ir, node_id));
        let focused = self.runtime.runtime_state.interaction.focused == Some(node_id);
        let mut flags = if semantics.disabled { 0 } else { FLAG_ENABLED };
        if semantics.focusable && !semantics.disabled {
            flags |= FLAG_FOCUSABLE;
        }
        if focused {
            flags |= FLAG_FOCUSED;
        }
        if semantics.checked.is_some() {
            flags |= FLAG_CHECKABLE;
        }
        if semantics.checked == Some(true) {
            flags |= FLAG_CHECKED;
        }
        if is_text_input_role(semantics.role) {
            flags |= FLAG_EDITABLE;
        }
        if semantics.masked {
            flags |= FLAG_PASSWORD;
        }
        if semantics.multiline || semantics.text_input_type == TextInputType::Multiline {
            flags |= FLAG_MULTILINE;
        }
        if semantics.scrollable_x {
            flags |= FLAG_SCROLL_X;
        }
        if semantics.scrollable_y {
            flags |= FLAG_SCROLL_Y;
        }
        if semantics.read_only {
            flags |= FLAG_READ_ONLY;
        }

        let mut actions = 0;
        if semantics
            .actions
            .entries
            .iter()
            .any(|entry| entry.trigger == ActionTrigger::Default)
            && !semantics.disabled
        {
            actions |= ACTION_CLICK;
        }
        if semantics.focusable && !semantics.disabled {
            actions |= ACTION_FOCUS;
        }
        if is_text_input_role(semantics.role) && !semantics.disabled && !semantics.read_only {
            actions |= ACTION_SET_TEXT | ACTION_SET_SELECTION;
        }
        if semantics.scrollable_x {
            actions |= ACTION_SCROLL_X;
        }
        if semantics.scrollable_y {
            actions |= ACTION_SCROLL_Y;
        }
        if numeric_range(semantics).is_some() {
            actions |= ACTION_INCREMENT | ACTION_DECREMENT;
        }

        let selection = if is_text_input_role(semantics.role) {
            let (anchor, caret) = self
                .runtime
                .runtime_state
                .text_edit
                .get(node_id)
                .map(|state| (state.anchor, state.caret))
                .or(semantics.text_selection)
                .unwrap_or((0, 0));
            [utf16_i32(&value, anchor), utf16_i32(&value, caret)]
        } else {
            [-1, -1]
        };

        AndroidSemanticsNode {
            id,
            parent_id,
            role: role_code(semantics),
            flags,
            actions,
            bounds: self.bounds(node_id),
            label,
            value: (!value.is_empty()).then_some(value),
            selection_utf16: selection,
            numeric: numeric_range(semantics).unwrap_or([f32::NAN; 3]),
        }
    }

    fn bounds(&self, node_id: WidgetId) -> [i32; 4] {
        self.layout
            .get_node_rect(node_id)
            .map(|rect| android_rect(rect, self.scale_factor))
            .unwrap_or_default()
    }

    fn node_id_for(&mut self, widget_id: WidgetId) -> i32 {
        if let Some(id) = self.widget_to_node.get(&widget_id) {
            return *id;
        }
        let raw = widget_id.as_u128();
        let folded = ((raw >> 96) ^ (raw >> 64) ^ (raw >> 32) ^ raw) as u32;
        let mut candidate = (folded & i32::MAX as u32) as i32;
        if candidate == 0 {
            candidate = 1;
        }
        while self.used_ids.contains(&candidate) {
            candidate = candidate.checked_add(1).unwrap_or(1);
        }
        self.used_ids.insert(candidate);
        self.widget_to_node.insert(widget_id, candidate);
        self.node_map.insert(candidate, widget_id);
        candidate
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

fn role_code(semantics: &Semantics) -> i32 {
    match semantics.role {
        Role::Generic => 0,
        Role::Button => 1,
        Role::Link => 2,
        Role::MenuItem => 3,
        Role::Text => 4,
        Role::TextInput | Role::Input => 5,
        Role::Image => 6,
        Role::Checkbox => 7,
        Role::Radio => 8,
        Role::Switch => 9,
        Role::Dialog => 10,
        Role::Slider => 11,
        Role::List => 12,
        Role::ListItem => 13,
    }
}

fn android_rect(rect: LayoutRect, scale_factor: f64) -> [i32; 4] {
    let coordinate = |value: f32| {
        ((value as f64 * scale_factor).round()).clamp(i32::MIN as f64, i32::MAX as f64) as i32
    };
    [
        coordinate(rect.x()),
        coordinate(rect.y()),
        coordinate(rect.x() + rect.width()),
        coordinate(rect.y() + rect.height()),
    ]
}

fn semantics_for(ir: &CoreIR, id: WidgetId) -> Option<&Semantics> {
    ir.nodes.get(&id).and_then(|node| match &node.op {
        Op::Semantics(semantics) => Some(semantics),
        _ => None,
    })
}

fn focused_semantics<'a>(runtime: &Runtime, ir: &'a CoreIR) -> Option<(WidgetId, &'a Semantics)> {
    let id = runtime.runtime_state.interaction.focused?;
    semantics_for(ir, id).map(|semantics| (id, semantics))
}

fn current_text_value(runtime: &Runtime, id: WidgetId, semantics: &Semantics) -> String {
    if is_text_input_role(semantics.role) {
        if let Some(state) = runtime.runtime_state.text_edit.get(id) {
            if state.pending_model_sync || semantics.value.is_none() {
                return state.committed_text();
            }
        }
        semantics.value.clone().unwrap_or_default()
    } else {
        semantics.value.clone().unwrap_or_default()
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
        Op::Paint(PaintOp::DrawText { text, .. }) => append_text(out, text),
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            append_text(
                out,
                &runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            );
        }
        _ => {
            for child in &node.children {
                collect_descendant_text_inner(ir, *child, out);
            }
        }
    }
}

fn append_text(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
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
        if let Some(semantics) = semantics_for(ir, old_id) {
            let _ = dispatch_semantics_action(
                runtime,
                old_id,
                semantics,
                ActionTrigger::Blur,
                ActionInput::None,
                None,
            );
        }
    }
    runtime.runtime_state.interaction.set_focused(focus);
    if let Some(handler) = &runtime.ime_handler {
        let allowed = focus
            .and_then(|id| semantics_for(ir, id))
            .map(|semantics| {
                is_text_input_role(semantics.role) && !semantics.disabled && !semantics.read_only
            })
            .unwrap_or(false);
        handler.set_ime_allowed(allowed);
    }
    if let Some(new_id) = focus {
        if let Some(semantics) = semantics_for(ir, new_id) {
            let _ = dispatch_semantics_action(
                runtime,
                new_id,
                semantics,
                ActionTrigger::Focus,
                ActionInput::None,
                None,
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
    fallback_payload: Option<Vec<u8>>,
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
        payload: entry
            .payload_data
            .clone()
            .or(fallback_payload)
            .unwrap_or_default(),
    };
    let input = if let Some(scope_id) = semantics.action_scope_id {
        ActionInput::scoped_raw(scope_id, target, input)
    } else {
        input
    };
    runtime
        .dispatch_with_input(envelope, target, &input)
        .is_ok()
}

fn set_text_input_value(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    semantics: &Semantics,
    value: &str,
) -> bool {
    replace_text_input_value(runtime, ir, target, semantics, value, None)
}

fn replace_text_input_value(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    semantics: &Semantics,
    value: &str,
    selection_utf16: Option<(usize, usize)>,
) -> bool {
    if !is_text_input_role(semantics.role) || semantics.disabled || semantics.read_only {
        return false;
    }
    set_focus(runtime, ir, Some(target));
    runtime.runtime_state.text_edit.sync_from_runtime(
        target,
        semantics.value.as_deref().unwrap_or_default(),
        None,
        None,
    );
    let (anchor, caret) = selection_utf16
        .map(|(start, end)| (utf16_to_byte(value, start), utf16_to_byte(value, end)))
        .unwrap_or((value.len(), value.len()));
    let committed = {
        let state = runtime.runtime_state.text_edit.get_mut_or_default(target);
        let old_len = state.buffer.len_bytes();
        state.apply_edit(0..old_len, value, caret, anchor)
    };
    let payload = serde_json::to_vec(&committed).ok();
    let mut changed =
        dispatch_dynamic_action(runtime, target, semantics, ActionTrigger::Change, payload);
    changed |= dispatch_cursor_change(runtime, target, semantics, caret, anchor);
    changed
}

fn set_text_selection_utf16(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    semantics: &Semantics,
    start_utf16: usize,
    end_utf16: usize,
) -> bool {
    if !is_text_input_role(semantics.role) || semantics.disabled {
        return false;
    }
    set_focus(runtime, ir, Some(target));
    runtime.runtime_state.text_edit.sync_from_runtime(
        target,
        semantics.value.as_deref().unwrap_or_default(),
        None,
        None,
    );
    let value = current_text_value(runtime, target, semantics);
    let anchor = utf16_to_byte(&value, start_utf16);
    let caret = utf16_to_byte(&value, end_utf16);
    let state = runtime.runtime_state.text_edit.get_mut_or_default(target);
    state.caret = caret;
    state.anchor = anchor;
    state.clear_preedit();
    dispatch_cursor_change(runtime, target, semantics, caret, anchor)
}

fn is_text_input_role(role: Role) -> bool {
    matches!(role, Role::TextInput | Role::Input)
}

fn dispatch_cursor_change(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    caret: usize,
    anchor: usize,
) -> bool {
    let payload = serde_json::to_vec(&fission_core::action::CursorChanged { caret, anchor }).ok();
    dispatch_dynamic_action(
        runtime,
        target,
        semantics,
        ActionTrigger::CursorChange,
        payload,
    )
}

fn adjust_numeric_value(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    direction: f32,
) -> bool {
    let Some([current, min, max]) = numeric_range(semantics) else {
        return false;
    };
    let next = (current + direction).clamp(min, max);
    set_numeric_value(runtime, target, semantics, next)
}

fn set_numeric_value(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    value: f32,
) -> bool {
    let Some([_, min, max]) = numeric_range(semantics) else {
        return false;
    };
    if !value.is_finite() {
        return false;
    }
    let value = value.clamp(min, max);
    let payload = serde_json::to_vec(&value).ok();
    dispatch_dynamic_action(runtime, target, semantics, ActionTrigger::Change, payload)
}

fn numeric_range(semantics: &Semantics) -> Option<[f32; 3]> {
    let current = semantics.current_value.filter(|value| value.is_finite())?;
    let min = semantics.min_value.unwrap_or_else(|| {
        (current - 1.0)
            .is_finite()
            .then_some(current - 1.0)
            .unwrap_or(current)
    });
    let max = semantics.max_value.unwrap_or_else(|| {
        (current + 1.0)
            .is_finite()
            .then_some(current + 1.0)
            .unwrap_or(current)
    });
    (min.is_finite() && max.is_finite() && min <= current && current <= max)
        .then_some([current, min, max])
}

fn dispatch_dynamic_action(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    trigger: ActionTrigger,
    payload: Option<Vec<u8>>,
) -> bool {
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == trigger)
    else {
        return false;
    };
    let input = if let Some(scope_id) = semantics.action_scope_id {
        ActionInput::scoped_raw(scope_id, target, ActionInput::None)
    } else {
        ActionInput::None
    };
    runtime
        .dispatch_with_input(
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload: payload.unwrap_or_default(),
            },
            target,
            &input,
        )
        .is_ok()
}

fn handle_scroll(
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    target: WidgetId,
    semantics: &Semantics,
    direction: i32,
) -> bool {
    let horizontal = direction.abs() == 2 || (semantics.scrollable_x && !semantics.scrollable_y);
    let Some(scroll_node) = find_scroll_node(ir, target, horizontal) else {
        return false;
    };
    let Some(geometry) = layout.get_node_geometry(scroll_node) else {
        return false;
    };
    let (extent, viewport) = if horizontal {
        (geometry.content_size.width, geometry.rect.width())
    } else {
        (geometry.content_size.height, geometry.rect.height())
    };
    let max = (extent - viewport).max(0.0);
    let current = runtime.runtime_state.scroll.get_offset(scroll_node);
    let next = (current + viewport * 0.8 * direction.signum() as f32).clamp(0.0, max);
    if (next - current).abs() <= 0.001 {
        return false;
    }
    runtime.runtime_state.scroll.set_offset(scroll_node, next);
    true
}

fn find_scroll_node(ir: &CoreIR, target: WidgetId, horizontal: bool) -> Option<WidgetId> {
    let direction = if horizontal {
        fission_ir::FlexDirection::Row
    } else {
        fission_ir::FlexDirection::Column
    };
    let mut stack = vec![target];
    while let Some(id) = stack.pop() {
        let node = ir.nodes.get(&id)?;
        if matches!(&node.op, Op::Layout(fission_ir::LayoutOp::Scroll { direction: value, .. }) if *value == direction)
        {
            return Some(id);
        }
        stack.extend(node.children.iter().rev().copied());
    }
    None
}

fn utf16_to_byte(value: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, ch) in value.char_indices() {
        if units >= offset {
            return index;
        }
        let next = units + ch.len_utf16();
        if next > offset {
            return index;
        }
        units = next;
    }
    value.len()
}

fn byte_to_utf16(value: &str, byte_offset: usize) -> usize {
    let mut clamped = byte_offset.min(value.len());
    while clamped > 0 && !value.is_char_boundary(clamped) {
        clamped -= 1;
    }
    value[..clamped].encode_utf16().count()
}

fn utf16_i32(value: &str, byte_offset: usize) -> i32 {
    i32::try_from(byte_to_utf16(value, byte_offset)).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_utf8_and_android_utf16_offsets() {
        let value = "a🌍b";
        assert_eq!(byte_to_utf16(value, 5), 3);
        assert_eq!(utf16_to_byte(value, 3), 5);
        assert_eq!(utf16_to_byte(value, 2), 1);
    }
}
