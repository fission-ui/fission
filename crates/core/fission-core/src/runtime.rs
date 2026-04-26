use crate::action::{Action, ActionEnvelope, ActionId, AppState};
use crate::effect::{ActionInput, EffectEnvelope};
use crate::env::{
    ActiveAnimation, AnimationStateMap, Env, InteractionStateMap, RuntimeState, ScrollStateMap,
    VideoStateMap, VideoStatus,
};
use crate::registry::{ActionRegistry, AnimationRequest, AnimationStartValue, VideoRegistration};
use crate::BoxedReducer;
use crate::{
    Clipboard, Clock, CurrentTime, ImeHandler, InputEvent, KeyCode, KeyEvent, PointerButton,
    PointerEvent,
};
use anyhow::{anyhow, Result};
use fission_diagnostics::prelude as diag;
use fission_ir::{CoreIR, FlexDirection, LayoutOp, NodeId, Op, WidgetNodeId};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot, LayoutUnit, TextMeasurer};
use glam::{Mat4, Vec4};
use serde_json;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The core runtime that owns application state, reducers, and the effect queue.
///
/// `Runtime` is the single entry point for the action/reducer pipeline. Platform
/// shells create one `Runtime`, register their `AppState`, build the widget tree
/// each frame, absorb the resulting [`ActionRegistry`], and call
/// [`Runtime::handle_input`] to process user events.
///
/// # Lifecycle
///
/// ```text
/// 1. runtime = Runtime::default()
///        .with_measurer(measurer)
///        .with_clipboard(clipboard);
/// 2. runtime.add_app_state(Box::new(MyState::default()))?;
/// 3. loop {
///        let mut ctx = BuildCtx::new();
///        let tree = my_widget.build(&mut ctx, &view);
///        runtime.clear_reducers();
///        runtime.absorb_registry(ctx.registry);
///        // lower tree -> IR -> layout -> render
///        runtime.handle_input(event, &ir, &layout)?;
///        runtime.tick(dt)?;
///    }
/// ```
///
/// # Example
///
/// ```rust,ignore
/// let mut runtime = Runtime::default();
/// runtime.add_app_state(Box::new(Counter { count: 0 }))?;
/// runtime.tick(16)?;
/// ```
pub struct Runtime {
    /// Per-frame reducers, cleared and re-populated every frame via
    /// [`absorb_registry`](Runtime::absorb_registry).
    pub reducers: HashMap<ActionId, Vec<BoxedReducer>>,
    /// Persistent reducers that survive [`clear_reducers`](Runtime::clear_reducers)
    /// calls, installed once at app startup.
    pub persistent_reducers: HashMap<ActionId, Vec<BoxedReducer>>,
    /// Type-indexed application state store.
    pub app_states: HashMap<TypeId, Box<dyn AppState>>,
    /// Mutable runtime state (interaction, scroll, text editing, animations).
    pub runtime_state: RuntimeState,
    /// Platform-provided text measurer for layout.
    pub measurer: Option<Arc<dyn TextMeasurer>>,
    /// Platform-provided clipboard backend.
    pub clipboard_backend: Option<Arc<dyn Clipboard>>,
    /// Platform-provided IME (Input Method Editor) handler.
    pub ime_handler: Option<Arc<dyn ImeHandler>>,
    /// Effects emitted by reducers, awaiting platform execution.
    pub pending_effects: Vec<EffectEnvelope>,
    /// Monotonically increasing counter for deterministic request id generation.
    pub next_req_id: u64,
}

impl Default for Runtime {
    fn default() -> Self {
        let mut runtime = Self {
            reducers: HashMap::new(),
            persistent_reducers: HashMap::new(),
            app_states: HashMap::new(),
            runtime_state: RuntimeState::default(),
            measurer: None,
            clipboard_backend: None,
            ime_handler: None,
            pending_effects: Vec::new(),
            next_req_id: 0,
        };

        runtime
            .add_app_state(Box::new(Clock::default()))
            .expect("Failed to add Clock state");

        runtime.register_base_reducers();

        runtime
    }
}

impl Runtime {
    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
    }

    pub fn with_clipboard(mut self, backend: Arc<dyn Clipboard>) -> Self {
        self.clipboard_backend = Some(backend);
        self
    }

    pub fn with_ime_handler(mut self, handler: Arc<dyn ImeHandler>) -> Self {
        self.ime_handler = Some(handler);
        self
    }

    pub fn caret_from_point_in_text(
        &self,
        value: &str,
        font_size: f32,
        viewport_x: f32,
        viewport_w: f32,
        content_w: f32,
        scroll_offset: f32,
        point_x: f32,
    ) -> usize {
        crate::input::text::caret_from_point_in_text(
            self.measurer.as_ref(),
            value,
            font_size,
            viewport_x,
            viewport_w,
            content_w,
            scroll_offset,
            point_x,
        )
    }

    // Helper for manual reducer registration (internal use)
    pub fn register_reducer<S: AppState + 'static>(
        &mut self,
        action_id: ActionId,
        reducer_fn: fn(&mut S, &ActionEnvelope, NodeId) -> Result<()>,
    ) -> Result<()> {
        let state_type_id = TypeId::of::<S>();

        // Wrap legacy 3-arg reducer into 5-arg BoxedReducer
        let boxed_reducer: BoxedReducer = Box::new(
            move |app_states: &mut HashMap<TypeId, Box<dyn AppState>>,
                  action: &ActionEnvelope,
                  target: NodeId,
                  _effects: &mut Vec<EffectEnvelope>,
                  _input: &ActionInput|
                  -> Result<()> {
                if let Some(state_box) = app_states.get_mut(&state_type_id) {
                    let concrete_state = state_box.downcast_mut::<S>().ok_or_else(|| {
                        anyhow!("Failed to downcast AppState to concrete type for reducer")
                    })?;
                    reducer_fn(concrete_state, action, target)
                } else {
                    anyhow::bail!("Target AppState for reducer not found in runtime.");
                }
            },
        );

        self.reducers
            .entry(action_id)
            .or_default()
            .push(boxed_reducer);
        Ok(())
    }

    pub fn register_base_reducers(&mut self) {
        use crate::{AdvanceTo, Tick, ADVANCE_TO_ACTION_ID, TICK_ACTION_ID};

        self.register_reducer::<Clock>(
            *TICK_ACTION_ID,
            |state: &mut Clock, action: &ActionEnvelope, _target| {
                let tick_action: Tick = serde_json::from_slice(&action.payload)
                    .map_err(|e| anyhow!("Failed to deserialize Tick: {}", e))?;
                state.advance_by(tick_action.dt)
            },
        )
        .expect("Failed to register Tick reducer");

        self.register_reducer::<Clock>(
            *ADVANCE_TO_ACTION_ID,
            |state: &mut Clock, action: &ActionEnvelope, _target| {
                let advance_action: AdvanceTo = serde_json::from_slice(&action.payload)
                    .map_err(|e| anyhow!("Failed to deserialize AdvanceTo: {}", e))?;
                state.set_to(advance_action.time)
            },
        )
        .expect("Failed to register AdvanceTo reducer");
    }

    pub fn clear_reducers(&mut self) {
        self.reducers.clear();
        self.register_base_reducers();
    }

    pub fn absorb_registry<S: AppState>(&mut self, registry: ActionRegistry<S>) {
        let new_reducers = registry.into_runtime_reducers();
        for (id, mut list) in new_reducers {
            self.reducers.entry(id).or_default().append(&mut list);
        }
    }

    /// Registers reducers that should survive `clear_reducers()` calls.
    ///
    /// This is intended for app-level "global" handlers (e.g. system effects) that
    /// are installed once at app startup, while per-frame widget handlers are
    /// regenerated every frame via `BuildCtx` and `absorb_registry`.
    pub fn absorb_persistent_registry<S: AppState>(&mut self, registry: ActionRegistry<S>) {
        let new_reducers = registry.into_runtime_reducers();
        for (id, mut list) in new_reducers {
            self.persistent_reducers
                .entry(id)
                .or_default()
                .append(&mut list);
        }
    }

    pub fn clock(&self) -> &Clock {
        self.get_app_state::<Clock>()
            .expect("Clock state must always be present")
    }

    pub fn get_app_state<S: AppState + 'static>(&self) -> Option<&S> {
        self.app_states
            .get(&TypeId::of::<S>())
            .and_then(|s_box| s_box.downcast_ref::<S>())
    }

    pub fn get_app_state_mut<S: AppState + 'static>(&mut self) -> Option<&mut S> {
        self.app_states
            .get_mut(&TypeId::of::<S>())
            .and_then(|s_box| s_box.downcast_mut::<S>())
    }

    pub fn add_app_state<S: AppState + 'static>(&mut self, state: Box<S>) -> Result<()> {
        let type_id = TypeId::of::<S>();
        if self.app_states.insert(type_id, state).is_some() {
            anyhow::bail!("App state of this type already registered.");
        }
        Ok(())
    }

    pub fn dispatch(&mut self, action: ActionEnvelope, target: NodeId) -> Result<()> {
        self.dispatch_with_input(action, target, &ActionInput::None)
    }

    pub fn dispatch_with_input(
        &mut self,
        action: ActionEnvelope,
        target: NodeId,
        input: &ActionInput,
    ) -> Result<()> {
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent {
                kind: "dispatch_start".into(),
                target: Some(target.as_u128()),
                position: None,
            },
        );

        // Delegate video actions to media module
        if crate::media::handle_video_action(&mut self.runtime_state.video, &action)? {
            return Ok(());
        }

        let action_id = action.id;

        // Collect effects from this dispatch (both persistent and per-frame reducers).
        let mut effects = Vec::new();

        if let Some(reducers) = self.persistent_reducers.get_mut(&action_id) {
            diag::emit(
                diag::DiagCategory::Input,
                diag::DiagLevel::Debug,
                diag::DiagEventKind::InputEvent {
                    kind: format!("persistent_reducers:{}", reducers.len()),
                    target: Some(target.as_u128()),
                    position: None,
                },
            );

            let mut temp_reducers: Vec<BoxedReducer> = reducers.drain(..).collect();
            for reducer_wrapper in temp_reducers.iter_mut() {
                reducer_wrapper(&mut self.app_states, &action, target, &mut effects, input)?;
            }
            reducers.extend(temp_reducers);
        }

        if let Some(reducers) = self.reducers.get_mut(&action_id) {
            diag::emit(
                diag::DiagCategory::Input,
                diag::DiagLevel::Debug,
                diag::DiagEventKind::InputEvent {
                    kind: format!("reducers:{}", reducers.len()),
                    target: Some(target.as_u128()),
                    position: None,
                },
            );

            let mut temp_reducers: Vec<BoxedReducer> = reducers.drain(..).collect();
            for reducer_wrapper in temp_reducers.iter_mut() {
                reducer_wrapper(&mut self.app_states, &action, target, &mut effects, input)?;
            }
            reducers.extend(temp_reducers);
        }

        // Process effects: Assign ReqIds and queue them
        for mut envelope in effects {
            // Assign deterministic ReqId
            envelope.req_id = self.next_req_id;
            self.next_req_id += 1;
            self.pending_effects.push(envelope);
        }

        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent {
                kind: "dispatch_end".into(),
                target: Some(target.as_u128()),
                position: None,
            },
        );
        Ok(())
    }

    pub fn tick(&mut self, dt: CurrentTime) -> Result<()> {
        use crate::Tick;
        let action = Tick { dt };
        let envelope: ActionEnvelope = action.into();
        self.dispatch(envelope, NodeId::derived(0, &[0]))?;

        let current_time = self.clock().current_time();

        let mut finished = Vec::new();
        let mut has_animation_changes = false;
        for ((target, property), anim) in self.runtime_state.animation.active.iter_mut() {
            let elapsed = current_time.saturating_sub(anim.start_time);
            let mut progress = if anim.duration == 0 {
                1.0
            } else {
                (elapsed as f32 / anim.duration as f32)
            };

            if anim.repeat && progress >= 1.0 {
                progress = progress % 1.0;
            } else {
                progress = progress.clamp(0.0, 1.0);
            }

            if !anim.repeat && (elapsed >= anim.duration || anim.duration == 0) {
                finished.push((*target, property.clone()));
            }

            let value = anim.start_value + (anim.end_value - anim.start_value) * progress;
            
            // Only update and mark dirty if the value actually changed
            let current_val = self.runtime_state.animation.values.get(&(*target, property.clone())).copied();
            if current_val != Some(value) {
                self.runtime_state
                    .animation
                    .values
                    .insert((*target, property.clone()), value);
                has_animation_changes = true;
            }
        }

        for key in finished {
            self.runtime_state.animation.active.remove(&key);
            has_animation_changes = true;
        }

        let _ = has_animation_changes;

        Ok(())
    }

    pub fn enqueue_animation(&mut self, target: WidgetNodeId, request: AnimationRequest) {
        let key = (target, request.property.clone());

        // Declarative deduplication: If we are already animating to this target, ignore the new request.
        if let Some(active) = self.runtime_state.animation.active.get(&key) {
            // Fuzzy float comparison
            if (active.end_value - request.to).abs() < 0.001
                && active.duration == request.duration_ms
                && active.repeat == request.repeat
            {
                // Continue existing animation
                return;
            }
        }

        let current_value = self
            .runtime_state
            .animation
            .values
            .get(&key)
            .copied()
            .unwrap_or_else(|| request.property.default_value());

        // If we are already at the target value and no animation is running, do we need to start one?
        // Yes, because we might want to ensure it's "set" or trigger completion events (if we had them).
        // But if start == end and duration > 0, it's a no-op animation?
        // Optimization: if current == to, maybe skip?
        // But if we want to "hold" the value, active animation keeps it?
        // Let's simpler logic: Start new if target changed.

        let start_value = match request.from {
            AnimationStartValue::Explicit(v) => v,
            AnimationStartValue::Current => current_value,
        };

        let anim = ActiveAnimation {
            target,
            property: request.property.clone(),
            start_value,
            end_value: request.to,
            start_time: self.clock().current_time() + request.delay_ms,
            duration: request.duration_ms,
            repeat: request.repeat,
        };

        self.runtime_state
            .animation
            .values
            .insert(key.clone(), start_value);
        self.runtime_state.animation.active.insert(key, anim);
    }

    pub fn sync_video_nodes(&mut self, registrations: &[VideoRegistration]) {
        let mut seen: HashSet<WidgetNodeId> = HashSet::new();

        for reg in registrations {
            seen.insert(reg.node_id);
            let entry = self
                .runtime_state
                .video
                .states
                .entry(reg.node_id)
                .or_insert_with(crate::env::VideoState::default);
            entry.asset_source = reg.source.clone();
            entry.looped = reg.loop_playback;
            if reg.autoplay && entry.status == VideoStatus::Stopped {
                entry.status = VideoStatus::Playing;
            }
        }

        self.runtime_state
            .video
            .states
            .retain(|node_id, _| seen.contains(node_id));
    }

    pub fn sync_web_nodes(&mut self, registrations: &[crate::registry::WebRegistration]) {
        let mut seen: HashSet<WidgetNodeId> = HashSet::new();

        for reg in registrations {
            seen.insert(reg.node_id);
            let entry = self
                .runtime_state
                .web
                .states
                .entry(reg.node_id)
                .or_insert_with(crate::env::WebState::default);

            // Only update URL if it changes to avoid reload loops
            if entry.url != reg.url {
                entry.url = reg.url.clone();
                entry.loading = true; // Assume loading starts
            }
            entry.user_agent = reg.user_agent.clone();
        }

        self.runtime_state
            .web
            .states
            .retain(|node_id, _| seen.contains(node_id));
    }

    pub fn post_layout_hook(&mut self, ir: &CoreIR, layout: &LayoutSnapshot) {
        let mut current_heroes = HashMap::new();

        for (id, node) in &ir.nodes {
            if let Op::Semantics(s) = &node.op {
                if let Some(tag) = &s.hero_tag {
                    if let Some(geom) = layout.get_node_geometry(*id) {
                        current_heroes.insert(tag.clone(), (*id, geom.rect));
                    }
                }
            }
        }

        // Detection logic for future flight animations
        for (tag, (_new_id, new_rect)) in &current_heroes {
            if let Some((_old_id, old_rect)) = self.runtime_state.hero.positions.get(tag) {
                if *new_rect != *old_rect {
                    // Logic to spawn overlay flight ghost would go here
                    diag::emit(
                        diag::DiagCategory::Layout,
                        diag::DiagLevel::Debug,
                        diag::DiagEventKind::AnchorPlacement {
                            widget: 0,
                            node: 0,
                            rect_x: old_rect.origin.x,
                            rect_y: old_rect.origin.y,
                            rect_w: old_rect.size.width,
                            rect_h: old_rect.size.height,
                            place_left: new_rect.origin.x,
                            place_top: new_rect.origin.y,
                            note: Some(format!("Hero flight: {}", tag)),
                        },
                    );
                }
            }
        }

        self.runtime_state.hero.positions = current_heroes;
    }

    pub fn handle_input(
        &mut self,
        event: InputEvent,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> Result<()> {
        use crate::hit_test::{
            find_neighbor_focus_node, find_next_focus_node, hit_test_with_scroll, FocusDirection,
        };
        use crate::input::gesture::GestureController;
        use crate::input::slider::SliderController;
        use crate::input::text::TextInputController;
        use crate::input::{ControllerContext, InputController};
        use crate::ui::custom_render::downcast_render_object;

        // --- Custom render object event handling (runs first) ----------------
        // For pointer events we hit-test, then walk up from the hit node to
        // check whether any ancestor carries a custom render object.  The
        // first one that returns `handled = true` short-circuits the entire
        // standard controller chain.
        if let Some(point) = Self::event_point(&event) {
            if let Some(hit_node_id) =
                hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
            {
                // Find the custom render object for this click.  Walk up from the
                // hit node first; if not found, check all registered render objects
                // by rect containment (the hit may be on a wrapper node above the
                // CustomNode's lowered subtree).
                let mut target_ro: Option<(NodeId, &fission_ir::AnyRenderObject)> = None;
                {
                    let mut walk = Some(hit_node_id);
                    while let Some(nid) = walk {
                        if let Some(ro) = ir.custom_render_objects.get(&nid) {
                            target_ro = Some((nid, ro));
                            break;
                        }
                        walk = ir.nodes.get(&nid).and_then(|n| n.parent);
                    }
                }
                if target_ro.is_none() {
                    for (ro_nid, ro) in &ir.custom_render_objects {
                        if let Some(rect) = layout.get_node_rect(*ro_nid) {
                            if rect.contains(point) {
                                target_ro = Some((*ro_nid, ro));
                                break;
                            }
                        }
                    }
                }

                if let Some((nid, any_ro)) = target_ro {
                    if let Some(render_obj) = downcast_render_object(any_ro) {
                        let mut node_rect = layout
                            .get_node_rect(nid)
                            .unwrap_or(LayoutRect::new(0.0, 0.0, 0.0, 0.0));
                        // Adjust node_rect by ancestor scroll offsets so it reflects
                        // the VISUAL position, matching the screen-coordinate click.
                        {
                            let mut walk = ir.nodes.get(&nid).and_then(|n| n.parent);
                            while let Some(pid) = walk {
                                if let Some(pnode) = ir.nodes.get(&pid) {
                                    if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) = &pnode.op {
                                        let off = self.runtime_state.scroll.get_offset(pid);
                                        match direction {
                                            fission_ir::FlexDirection::Row => node_rect.origin.x -= off,
                                            fission_ir::FlexDirection::Column => node_rect.origin.y -= off,
                                        }
                                    }
                                    walk = pnode.parent;
                                } else { break; }
                            }
                        }
                        let result = render_obj.handle_event(nid, &event, node_rect);
                        if result.handled {
                            // Set focus to this node so keyboard events route here
                            if matches!(event, InputEvent::Pointer(PointerEvent::Down { .. })) {
                                self.runtime_state.interaction.set_focused(Some(nid));
                            }
                            // Dispatch any actions the render object produced.
                            for (target, envelope) in result.actions {
                                self.dispatch(envelope, target)?;
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }

        // --- Keyboard events → focused node's custom render object -----------
        // Keyboard events have no point, so we route them to the focused node
        // (if any) and walk up its ancestor chain looking for a custom render
        // object.  This allows custom editor nodes to handle arrow keys,
        // typing, etc. before the framework's default focus-navigation logic.
        if matches!(event, InputEvent::Keyboard(_)) {
            if let Some(focused_id) = self.runtime_state.interaction.focused {
                let mut walk_id = Some(focused_id);
                while let Some(nid) = walk_id {
                    if let Some(any_ro) = ir.custom_render_objects.get(&nid) {
                        if let Some(render_obj) = downcast_render_object(any_ro) {
                            let node_rect = layout
                                .get_node_rect(nid)
                                .unwrap_or(LayoutRect::new(0.0, 0.0, 0.0, 0.0));
                            let result = render_obj.handle_event(nid, &event, node_rect);
                            if result.handled {
                                for (target, envelope) in result.actions {
                                    self.dispatch(envelope, target)?;
                                }
                                return Ok(());
                            }
                        }
                    }
                    walk_id = ir.nodes.get(&nid).and_then(|n| n.parent);
                }
            }
        }

        let mut dispatched_actions = Vec::new();
        let mut handled = false;

        {
            let mut ctx = ControllerContext {
                ir,
                layout,
                text_edit: &mut self.runtime_state.text_edit,
                interaction: &mut self.runtime_state.interaction,
                scroll: &mut self.runtime_state.scroll,
                ime_preedit: &mut self.runtime_state.ime_preedit,
                gesture: &mut self.runtime_state.gesture,
                clipboard: self.clipboard_backend.as_ref(),
                measurer: self.measurer.as_ref(),
                dispatched_actions: Vec::new(),
            };

            let mut gesture_controller = GestureController;
            if gesture_controller.handle_event(&mut ctx, &event) {
                handled = true;
            } else {
                let mut text_controller = TextInputController;
                if text_controller.handle_event(&mut ctx, &event) {
                    handled = true;
                } else {
                    let mut slider_controller = SliderController;
                    if slider_controller.handle_event(&mut ctx, &event) {
                        handled = true;
                    }
                }
            }
            dispatched_actions = ctx.dispatched_actions;
        }

        for (target, action, input) in dispatched_actions {
            self.dispatch_with_input(action, target, &input)?;
        }

        if handled {
            if matches!(event, InputEvent::Pointer(PointerEvent::Up { .. })) {
                self.runtime_state.interaction.pressed.clear();
                self.runtime_state.interaction.last_down_point = None;
            }
            return Ok(());
        }

        match event {
            InputEvent::Pointer(PointerEvent::Scroll { point, delta }) => {
                let trace_scroll =
                    std::env::var("FISSION_SCROLL_TRACE").ok().as_deref() == Some("1");
                if trace_scroll {
                    eprintln!(
                        "[scroll-trace] event point=({:.1},{:.1}) delta=({:.1},{:.1})",
                        point.x, point.y, delta.x, delta.y
                    );
                }
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    if trace_scroll {
                        eprintln!("[scroll-trace] hit_node={}", hit_node_id.as_u128());
                    }
                    let mut current_id = Some(hit_node_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
                                let current_offset = self.runtime_state.scroll.get_offset(node_id);
                                let delta_val = match direction {
                                    FlexDirection::Row => delta.x,
                                    FlexDirection::Column => delta.y,
                                };
                                let mut new_offset = current_offset + delta_val;

                                let mut max_offset = 0.0f32;
                                let mut viewport_w = 0.0f32;
                                let mut viewport_h = 0.0f32;
                                let mut content_w = 0.0f32;
                                let mut content_h = 0.0f32;
                                if let Some(geom) = layout.get_node_geometry(node_id) {
                                    viewport_w = geom.rect.width();
                                    viewport_h = geom.rect.height();
                                    content_w = geom.content_size.width;
                                    content_h = geom.content_size.height;
                                    max_offset = if matches!(direction, FlexDirection::Row) {
                                        (geom.content_size.width - geom.rect.width()).max(0.0)
                                    } else {
                                        (geom.content_size.height - geom.rect.height()).max(0.0)
                                    };
                                    new_offset = new_offset.clamp(0.0, max_offset);
                                }

                                if trace_scroll {
                                    eprintln!(
                                        "[scroll-trace] scroll_node={} axis={} offset={:.1}->{:.1} max={:.1} viewport=({:.1},{:.1}) content=({:.1},{:.1})",
                                        node_id.as_u128(),
                                        match direction { FlexDirection::Row => "x", FlexDirection::Column => "y" },
                                        current_offset,
                                        new_offset,
                                        max_offset,
                                        viewport_w,
                                        viewport_h,
                                        content_w,
                                        content_h
                                    );
                                }

                                {
                                    use fission_diagnostics::prelude as diag;
                                    diag::emit(
                                        diag::DiagCategory::Input,
                                        diag::DiagLevel::Debug,
                                        diag::DiagEventKind::ScrollUpdate {
                                            node: node_id.as_u128(),
                                            axis: match direction {
                                                FlexDirection::Row => "x".into(),
                                                FlexDirection::Column => "y".into(),
                                            },
                                            point_x: point.x,
                                            point_y: point.y,
                                            delta: delta_val,
                                            old_offset: current_offset,
                                            new_offset,
                                            max_offset,
                                            viewport_w,
                                            viewport_h,
                                            content_w,
                                            content_h,
                                        },
                                    );
                                }

                                self.runtime_state.scroll.set_offset(node_id, new_offset);
                                // If scroll actually changed, consume the event.
                                // If it didn't (clamped to same value, e.g. max_offset==0),
                                // propagate to parent scroll nodes.
                                if (new_offset - current_offset).abs() > 0.001 {
                                    break;
                                }
                                // Fall through to parent
                            }
                            current_id = node.parent;
                        } else {
                            break;
                        }
                    }
                } else if trace_scroll {
                    eprintln!("[scroll-trace] hit_test: no node");
                }
            }
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => match key_code {
                KeyCode::Tab => {
                    let reverse = (modifiers & 1) != 0;
                    let old_focus = self.runtime_state.interaction.focused;
                    let next =
                        find_next_focus_node(ir, self.runtime_state.interaction.focused, reverse);
                    if next != old_focus {
                        self.runtime_state.ime_preedit = None;
                        self.clear_text_pending_on_blur(old_focus, next);
                    }
                    self.runtime_state.interaction.set_focused(next);
                }
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    if let Some(focused) = self.runtime_state.interaction.focused {
                        let dir = match key_code {
                            KeyCode::Up => FocusDirection::Up,
                            KeyCode::Down => FocusDirection::Down,
                            KeyCode::Left => FocusDirection::Left,
                            KeyCode::Right => FocusDirection::Right,
                            _ => unreachable!(),
                        };
                        if let Some(next) = find_neighbor_focus_node(ir, layout, focused, dir) {
                            self.runtime_state.ime_preedit = None;
                            self.clear_text_pending_on_blur(Some(focused), Some(next));
                            self.runtime_state.interaction.set_focused(Some(next));
                        }
                    }
                }
                KeyCode::Enter | KeyCode::Space => {
                    if let Some(focused_id) = self.runtime_state.interaction.focused {
                        let mut current_id = Some(focused_id);
                        while let Some(node_id) = current_id {
                            if let Some(node) = ir.nodes.get(&node_id) {
                                if let Op::Semantics(semantics) = &node.op {
                                    if let Some(action_entry) = semantics.actions.entries.first() {
                                        if let Some(payload) = &action_entry.payload_data {
                                            let envelope = ActionEnvelope {
                                                id: ActionId::from_u128(action_entry.action_id),
                                                payload: payload.clone(),
                                            };
                                            return self.dispatch(envelope, node_id);
                                        }
                                    }
                                }
                                current_id = node.parent;
                            } else {
                                break;
                            }
                        }
                    }
                }
                _ => {}
            },
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    diag::emit(
                        diag::DiagCategory::Input,
                        diag::DiagLevel::Debug,
                        diag::DiagEventKind::InputEvent {
                            kind: "pointer_down_hit".into(),
                            target: Some(hit_node_id.as_u128()),
                            position: Some((point.x, point.y)),
                        },
                    );
                    let mut focus_candidate = Some(hit_node_id);
                    while let Some(node_id) = focus_candidate {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Semantics(s) = &node.op {
                                if s.focusable {
                                    let old_focused_id = self.runtime_state.interaction.focused;
                                    if Some(node_id) != old_focused_id {
                                        self.runtime_state.ime_preedit = None;
                                        self.clear_text_pending_on_blur(
                                            old_focused_id,
                                            Some(node_id),
                                        );

                                        if s.role == fission_ir::semantics::Role::TextInput {
                                            if let Some(ime_handler) = &self.ime_handler {
                                                ime_handler.set_ime_allowed(true);
                                            }
                                        } else if let Some(ime_handler) = &self.ime_handler {
                                            ime_handler.set_ime_allowed(false);
                                        }
                                    }
                                    self.runtime_state.interaction.set_focused(Some(node_id));
                                    break;
                                }
                            }
                            focus_candidate = node.parent;
                        } else {
                            break;
                        }
                    }
                    if focus_candidate.is_none() {
                        let old_focused_id = self.runtime_state.interaction.focused;
                        if let Some(old_focused_id) = self.runtime_state.interaction.focused {
                            if let Some(old_node) = ir.nodes.get(&old_focused_id) {
                                if let Op::Semantics(s) = &old_node.op {
                                    if s.role == fission_ir::semantics::Role::TextInput {
                                        if let Some(ime_handler) = &self.ime_handler {
                                            ime_handler.set_ime_allowed(false);
                                        }
                                    }
                                }
                            }
                        }
                        self.clear_text_pending_on_blur(old_focused_id, None);
                        self.runtime_state.interaction.set_focused(None);
                    }

                    let mut current_pressed_id = Some(hit_node_id);
                    while let Some(node_id) = current_pressed_id {
                        self.runtime_state.interaction.set_pressed(node_id, true);
                        if let Some(node) = ir.nodes.get(&node_id) {
                            current_pressed_id = node.parent;
                        } else {
                            break;
                        }
                    }
                    self.runtime_state.interaction.last_down_point = Some(point);

                    if let Some(focused_id) = self.runtime_state.interaction.focused {
                        if let Some(node) = ir.nodes.get(&focused_id) {
                            if let Op::Semantics(s) = &node.op {
                                if s.role == fission_ir::semantics::Role::TextInput {
                                    if let Some(ime_handler) = &self.ime_handler {
                                        ime_handler.set_ime_cursor_area(LayoutRect::new(
                                            point.x, point.y, 2.0, 16.0,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    let old_focused_id = self.runtime_state.interaction.focused;
                    if let Some(old_focused_id) = self.runtime_state.interaction.focused {
                        if let Some(old_node) = ir.nodes.get(&old_focused_id) {
                            if let Op::Semantics(s) = &old_node.op {
                                if s.role == fission_ir::semantics::Role::TextInput {
                                    if let Some(ime_handler) = &self.ime_handler {
                                        ime_handler.set_ime_allowed(false);
                                    }
                                }
                            }
                        }
                    }
                    self.clear_text_pending_on_blur(old_focused_id, None);
                    self.runtime_state.interaction.set_focused(None);
                }
            }
            InputEvent::Pointer(PointerEvent::Up { point, .. }) => {
                self.runtime_state.interaction.pressed.clear();
                self.runtime_state.interaction.last_down_point = None;
                if let Some(hit_node_id) =
                    hit_test_with_scroll(ir, layout, &self.runtime_state.scroll, point)
                {
                    let mut current_id = Some(hit_node_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ir.nodes.get(&node_id) {
                            if let Op::Semantics(semantics) = &node.op {
                                if semantics.role == fission_ir::semantics::Role::TextInput {
                                    // No action
                                } else if let Some(action_entry) = semantics.actions.entries.first()
                                {
                                    if let Some(payload) = &action_entry.payload_data {
                                        let envelope = ActionEnvelope {
                                            id: ActionId::from_u128(action_entry.action_id),
                                            payload: payload.clone(),
                                        };
                                        diag::emit(
                                            diag::DiagCategory::Input,
                                            diag::DiagLevel::Debug,
                                            diag::DiagEventKind::InputEvent {
                                                kind: "pointer_up_dispatch".into(),
                                                target: Some(node_id.as_u128()),
                                                position: Some((point.x, point.y)),
                                            },
                                        );
                                        return self.dispatch(envelope, node_id);
                                    }
                                }
                            }
                            current_id = node.parent;
                        } else {
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn clear_text_pending_on_blur(&mut self, old_focus: Option<NodeId>, new_focus: Option<NodeId>) {
        if old_focus == new_focus {
            return;
        }
        if let Some(old_id) = old_focus {
            if let Some(st) = self.runtime_state.text_edit.states.get_mut(&old_id) {
                st.pending_model_sync = false;
            }
        }
    }

    pub fn hit_test(
        &self,
        point: LayoutPoint,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
    ) -> Option<NodeId> {
        if let Some(root) = ir.root {
            return self.hit_test_recursive(root, point, ir, snapshot);
        }
        None
    }

    fn hit_test_recursive(
        &self,
        node_id: NodeId,
        point: LayoutPoint,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
    ) -> Option<NodeId> {
        if let Some(geom) = snapshot.nodes.get(&node_id) {
            if geom.rect.contains(point) {
                if let Some(node) = ir.nodes.get(&node_id) {
                    for child in node.children.iter().rev() {
                        let mut child_point = point;

                        if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
                            if !geom.rect.contains(point) {
                                continue;
                            }
                            let offset = self.runtime_state.scroll.get_offset(node_id);
                            match direction {
                                FlexDirection::Row => child_point.x += offset,
                                FlexDirection::Column => child_point.y += offset,
                            }
                        }

                        if let Op::Layout(LayoutOp::Transform { transform }) = &node.op {
                            let mat = Mat4::from_cols_array(transform);
                            // We need to transform the point relative to the node's origin?
                            // Layout coordinates are relative to the parent.
                            // In hit_test_recursive, `point` is relative to current `node_id`?
                            // No, `point` is relative to the `geom.rect.origin` of `node_id`?
                            // Let's check recursion.

                            // hit_test starts at root with absolute point.
                            // recursion: `child_point = point`.
                            // wait, `hit_test_recursive` doesn't subtract location?
                            // Ah, I see: `if geom.rect.contains(point)`.
                            // This implies `point` is ABSOLUTE.

                            // If `point` is absolute, and we want to transform into child local space:
                            // 1. Move point to node local space: `point - node_pos`.
                            // 2. Apply inverse transform.
                            // 3. (Implicitly) Move back or keep local?
                            // Recursive call expects absolute point?
                            // No, `hit_test_recursive` calls itself with `child_point`.
                            // If it expects absolute point, then `Transform` node doesn't work well with absolute recursion.

                            // Actually, my `hit_test_recursive` impl seems to assume absolute points for all nodes?
                            // `if geom.rect.contains(point)` confirms it.

                            // So if I have a Transform, I MUST return a point that looks "absolute" to the child
                            // but is logically transformed.
                            // Absolute child rect is NOT transformed by LayoutEngine.

                            // This means `geom.rect` for children of a Transform is WRONG if they are visually moved.
                            // BUT LayoutEngine doesn't know about Matrix4.
                            // So the children think they are at `(0,0)` relative to parent.

                            // To make hit test work:
                            // 1. Convert absolute `point` to `node_local_point`.
                            // 2. Apply inverse transform to `node_local_point` -> `transformed_local_point`.
                            // 3. Convert `transformed_local_point` back to absolute for children -> `transformed_absolute_point`.

                            let local_x = point.x - geom.rect.origin.x;
                            let local_y = point.y - geom.rect.origin.y;

                            let p = Vec4::new(local_x, local_y, 0.0, 1.0);
                            let inv = mat.inverse();
                            let transformed = inv * p;

                            child_point = LayoutPoint::new(
                                transformed.x + geom.rect.origin.x,
                                transformed.y + geom.rect.origin.y,
                            );
                        }

                        if let Some(hit) =
                            self.hit_test_recursive(*child, child_point, ir, snapshot)
                        {
                            return Some(hit);
                        }
                    }

                    match &node.op {
                        Op::Paint(_)
                        | Op::Layout(LayoutOp::Scroll { .. })
                        | Op::Layout(LayoutOp::Embed { .. }) => return Some(node_id),
                        _ => return None,
                    }
                }
                return None;
            }
        }
        None
    }

    /// Extract the pointer position from an input event, if applicable.
    ///
    /// Used by the custom-render-object event dispatch to perform a hit-test
    /// before delegating to render objects.  Returns `None` for keyboard and
    /// other non-positional events.
    fn event_point(event: &InputEvent) -> Option<LayoutPoint> {
        match event {
            InputEvent::Pointer(PointerEvent::Down { point, .. })
            | InputEvent::Pointer(PointerEvent::Up { point, .. })
            | InputEvent::Pointer(PointerEvent::Move { point, .. })
            | InputEvent::Pointer(PointerEvent::Scroll { point, .. }) => Some(*point),
            _ => None,
        }
    }
}
