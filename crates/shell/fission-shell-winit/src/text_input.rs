use super::*;

pub(super) fn focused_text_input_id(runtime: &Runtime, ir: Option<&CoreIR>) -> Option<WidgetId> {
    let focused = runtime.runtime_state.interaction.focused?;
    let ir = ir?;
    let mut current = Some(focused);
    while let Some(id) = current {
        let node = ir.nodes.get(&id)?;
        if let Op::Semantics(sem) = &node.op {
            if matches!(
                sem.role,
                fission_ir::Role::TextInput | fission_ir::Role::Input
            ) {
                return Some(id);
            }
        }
        current = node.parent;
    }
    None
}

pub(super) fn focused_text_input_config(
    runtime: &Runtime,
    ir: Option<&CoreIR>,
) -> Option<TextInputConfig> {
    let id = focused_text_input_id(runtime, ir)?;
    let ir = ir?;
    let node = ir.nodes.get(&id)?;
    match &node.op {
        Op::Semantics(semantics) => {
            let config = TextInputConfig::from_semantics(semantics);
            #[cfg(any(target_os = "android", target_arch = "wasm32"))]
            let config = {
                let mut config = config;
                if let Some(state) = runtime.runtime_state.text_edit.get(id) {
                    config.value = state.committed_text();
                    config.selection = (state.anchor, state.caret);
                    config.preedit_active = state.preedit.is_some();
                }
                config
            };
            Some(config)
        }
        _ => None,
    }
}

pub(super) fn focused_custom_text_input(runtime: &Runtime, ir: Option<&CoreIR>) -> bool {
    let focused = match runtime.runtime_state.interaction.focused {
        Some(id) => id,
        None => return false,
    };
    let ir = match ir {
        Some(ir) => ir,
        None => return false,
    };
    let mut current = Some(focused);
    while let Some(id) = current {
        if let Some(any_ro) = ir.custom_render_objects.get(&id) {
            if let Some(render_obj) = downcast_render_object(any_ro) {
                if render_obj.accepts_text_input() {
                    return true;
                }
            }
        }
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    false
}

pub(super) fn reset_text_input_caret(
    runtime: &mut Runtime,
    ir: Option<&CoreIR>,
    last_blink_toggle: &mut Instant,
) {
    if let Some(id) = focused_text_input_id(runtime, ir) {
        runtime.runtime_state.caret_visible.insert(id, true);
        *last_blink_toggle = Instant::now();
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingTextTrace {
    pub(super) seq: u64,
    source: String,
    target: Option<WidgetId>,
    pub(super) started_at: Instant,
    pub(super) handled_at: Option<Instant>,
    pub(super) effects_at: Option<Instant>,
    present_after_frame: u64,
}

pub(super) fn start_text_trace(
    enabled: bool,
    traces: &mut VecDeque<PendingTextTrace>,
    next_seq: &mut u64,
    source: String,
    target: Option<WidgetId>,
    presented_frames: u64,
) -> Option<u64> {
    if !enabled {
        return None;
    }
    *next_seq += 1;
    let seq = *next_seq;
    traces.push_back(PendingTextTrace {
        seq,
        source,
        target,
        started_at: Instant::now(),
        handled_at: None,
        effects_at: None,
        present_after_frame: presented_frames + 1,
    });
    Some(seq)
}

pub(super) fn mark_text_trace_handled(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.handled_at = Some(Instant::now());
        }
    }
}

pub(super) fn mark_text_trace_effects(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.effects_at = Some(Instant::now());
        }
    }
}

pub(super) fn set_text_trace_target(
    traces: &mut VecDeque<PendingTextTrace>,
    seq: Option<u64>,
    target: Option<WidgetId>,
) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.target = target;
        }
    }
}

pub(super) fn cancel_text_trace(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        traces.retain(|trace| trace.seq != seq);
    }
}

pub(super) fn flush_text_traces(
    enabled: bool,
    traces: &mut VecDeque<PendingTextTrace>,
    presented_frames: u64,
) {
    if !enabled {
        traces.clear();
        return;
    }

    loop {
        let should_flush = traces
            .front()
            .map(|trace| trace.present_after_frame <= presented_frames)
            .unwrap_or(false);
        if !should_flush {
            break;
        }

        let Some(trace) = traces.pop_front() else {
            break;
        };
        let now = Instant::now();
        let handled_at = trace.handled_at.unwrap_or(now);
        let effects_at = trace.effects_at.unwrap_or(handled_at);
        let total_ms = now.duration_since(trace.started_at).as_secs_f64() * 1000.0;
        let handle_ms = handled_at.duration_since(trace.started_at).as_secs_f64() * 1000.0;
        let effects_ms = effects_at.duration_since(handled_at).as_secs_f64() * 1000.0;
        let queue_ms = now.duration_since(effects_at).as_secs_f64() * 1000.0;

        let target_u128 = trace.target.map(|id| id.as_u128());
        let msg = format!(
            "text_input_latency seq={} src={} handle_ms={:.2} effects_ms={:.2} queue_ms={:.2} total_ms={:.2} frame={}",
            trace.seq, trace.source, handle_ms, effects_ms, queue_ms, total_ms, presented_frames
        );
        eprintln!("[text-trace] {}", msg);
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Info,
            diag::DiagEventKind::InputEvent {
                kind: msg,
                target: target_u128,
                position: None,
            },
        );
    }
}

// ─── Extracted handler functions ─────────────────────────────────────────
// These are called by BOTH real WindowEvent handlers AND the TestEvent (UserEvent)
// handler, ensuring test infrastructure exercises the exact same code paths.
