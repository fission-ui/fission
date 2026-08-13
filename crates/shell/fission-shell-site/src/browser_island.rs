use crate::{render_ir_to_html_with_styles, CssVariableMap, HtmlRenderOptions, StyleRegistry};
use anyhow::{anyhow, Context, Result};
use fission_core::internal::BuildCtx;
use fission_core::internal::InternalLoweringCx;
use fission_core::registry::{VideoRegistration, WebRegistration};
use fission_core::ui::{Overlay, ZStack};
use fission_core::{
    ActionEnvelope, ActionId, Env, GlobalState, Runtime, RuntimeState, View, Widget, WidgetId,
};
use fission_ir::{semantics::ActionTrigger, CoreIR, Op, Role, Semantics};
use fission_theme::Theme;
use serde_json::{json, Value};
use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    static BROWSER_ISLANDS: RefCell<BTreeMap<String, Box<dyn Any>>> = RefCell::new(BTreeMap::new());
}

/// A focused browser-side Fission widget tree mounted into a server-rendered
/// page region.
///
/// The server shell compiles one WASM artifact per declared island. Each
/// artifact keeps its own state in browser memory, runs normal Fission
/// reducers for actions emitted by its widget tree, and returns renderer-owned
/// DOM patches to the server browser bridge.
pub struct BrowserIslandApp<S, W>
where
    S: GlobalState,
    W: Clone + Into<Widget>,
{
    id: String,
    mount_id: String,
    runtime: Runtime,
    widget: W,
    theme: Theme,
    _state: std::marker::PhantomData<fn() -> S>,
}

impl<S, W> BrowserIslandApp<S, W>
where
    S: GlobalState,
    W: Clone + Into<Widget>,
{
    /// Creates a browser island rooted at a semantic mount point.
    ///
    /// `id` identifies the route-local island instance and is used for
    /// diagnostics and generated stylesheet ids. `mount_id` must match the
    /// semantic identifier rendered by the server page, because the browser
    /// bridge replaces that region with the island's current widget output.
    pub fn new(id: impl Into<String>, mount_id: impl Into<String>, state: S, widget: W) -> Self {
        let runtime = Runtime::default().with_global_state(state);
        Self {
            id: id.into(),
            mount_id: mount_id.into(),
            runtime,
            widget,
            theme: Theme::default(),
            _state: std::marker::PhantomData,
        }
    }

    /// Uses a non-default theme when rendering the island into browser HTML.
    ///
    /// Pass the same theme as the surrounding route when the island should
    /// visually blend into the server-rendered page. If omitted, the default
    /// Fission theme is used.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    fn handle(&mut self, input: &str) -> Result<String> {
        let message = parse_bridge_message(input);
        if is_action_event(&message) {
            self.dispatch_browser_action(&message)?;
        }
        self.render_bridge_output(message.get("sequence").and_then(Value::as_u64).unwrap_or(1))
    }

    fn dispatch_browser_action(&mut self, message: &Value) -> Result<()> {
        if is_browser_text_action(message) {
            return self.dispatch_browser_text_action(message);
        }
        let action = message
            .get("binding")
            .and_then(|binding| binding.get("message"))
            .ok_or_else(|| anyhow!("browser island event is missing action metadata"))?;
        let action_id = action
            .get("action_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("browser island event is missing action_id"))?
            .parse::<u128>()
            .context("browser island action_id is not a u128")?;
        let target = action
            .get("target_node")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("browser island event is missing target_node"))?
            .parse::<u128>()
            .context("browser island target_node is not a u128")?;
        let payload = action
            .get("payload_hex")
            .and_then(Value::as_str)
            .map(hex_decode)
            .transpose()?
            .unwrap_or_default();

        let output = self.build_widget();
        self.install_registry(output.registry);
        let dispatch = self.runtime.dispatch(
            ActionEnvelope {
                id: ActionId::from_u128(action_id),
                payload,
            },
            WidgetId::from_u128(target),
        );
        self.finish_browser_dispatch(dispatch)?;
        Ok(())
    }

    fn dispatch_browser_text_action(&mut self, message: &Value) -> Result<()> {
        let action = message
            .get("binding")
            .and_then(|binding| binding.get("message"))
            .ok_or_else(|| anyhow!("browser island text event is missing action metadata"))?;
        let target = action
            .get("target_node")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("browser island text event is missing target_node"))?
            .parse::<u128>()
            .context("browser island text target_node is not a u128")?;
        let target = WidgetId::from_u128(target);
        let new_text = message
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("browser island text event is missing value"))?
            .to_string();
        let new_caret = bridge_text_offset(message, "caret", &new_text)?;
        let new_anchor = bridge_text_offset(message, "anchor", &new_text)?;

        let output = self.build_widget();
        let ir = lower_browser_island_widget(
            output.node,
            output.portals,
            &self.theme,
            &self.runtime.runtime_state,
        );
        let semantics = ir
            .nodes
            .get(&target)
            .and_then(|node| match &node.op {
                Op::Semantics(semantics) => Some(semantics),
                _ => None,
            })
            .ok_or_else(|| anyhow!("browser island text target {target} is not a semantic node"))?;
        validate_browser_text_target(target, semantics)?;
        let (envelope, input) = fission_core::input::prepare_scoped_text_input_change(
            &ir, semantics, target, new_text, new_caret, new_anchor,
        )
        .ok_or_else(|| anyhow!("browser island text target {target} has no text-change action"))?;
        self.install_registry(output.registry);
        let dispatch = self.runtime.dispatch_with_input(envelope, target, &input);
        self.finish_browser_dispatch(dispatch)?;
        Ok(())
    }

    fn render_bridge_output(&mut self, sequence: u64) -> Result<String> {
        let output = self.build_widget();
        let ir = lower_browser_island_widget(
            output.node,
            output.portals,
            &self.theme,
            &self.runtime.runtime_state,
        );

        let mut styles = StyleRegistry::default();
        let rendered = render_ir_to_html_with_styles(
            &ir,
            &HtmlRenderOptions {
                document_title: self.id.clone(),
                root_class: "fission-browser-island-root".to_string(),
                css_variables: CssVariableMap::from_theme(&self.theme),
                browser_action_bindings: true,
                motion_declarations: output.motion_declarations,
                video_registrations: output
                    .video_registrations
                    .into_iter()
                    .map(|registration| (registration.node_id, registration))
                    .collect(),
                web_registrations: output
                    .web_registrations
                    .into_iter()
                    .map(|registration| (registration.node_id, registration))
                    .collect(),
                ..Default::default()
            },
            &mut styles,
        )?;
        Ok(json!({
            "messages": [
                {
                    "type": "dom_batch",
                    "sequence": sequence,
                    "transaction_id": self.id,
                    "ops": [
                        {
                            "op": "set_stylesheet",
                            "id": format!("fission-island-{}", self.id),
                            "css": rendered.css
                        },
                        {
                            "op": "replace_children_html_by_semantics",
                            "semantics": self.mount_id,
                            "html": rendered.body_html
                        },
                        {
                            "op": "set_attr_by_semantics",
                            "semantics": self.mount_id,
                            "name": "data-fission-island-loaded",
                            "value": "true"
                        }
                    ]
                }
            ],
            "bindings": []
        })
        .to_string())
    }

    fn build_widget(&self) -> BrowserIslandBuildOutput<S> {
        let mut env = Env::default();
        env.theme = self.theme.clone();
        let state = self
            .runtime
            .get_global_state::<S>()
            .expect("browser island state is registered at construction");
        let view = View::new(state, &self.runtime.runtime_state, &env, None);
        let mut ctx = BuildCtx::<S>::new();
        let node = fission_core::build::enter(&mut ctx, &view, || self.widget.clone().into());
        let motion_declarations = ctx.take_motion_declarations();
        let video_registrations = ctx.take_video_registrations();
        let web_registrations = ctx.take_web_registrations();
        let portals = ctx.take_portals();
        let registry = ctx.registry;
        BrowserIslandBuildOutput {
            node,
            registry,
            motion_declarations,
            video_registrations,
            web_registrations,
            portals,
        }
    }

    fn install_registry(&mut self, registry: fission_core::registry::ActionRegistry<S>) {
        self.runtime.clear_reducers();
        self.runtime.absorb_registry(registry);
    }

    fn finish_browser_dispatch(&mut self, dispatch: Result<()>) -> Result<()> {
        let discarded_effects = self.runtime.discard_pending_effects();
        dispatch?;
        if discarded_effects > 0 {
            anyhow::bail!(
                "browser island `{}` reducer queued unsupported effects; discarded {discarded_effects} effect envelope(s) and completion callback(s)",
                self.id
            );
        }
        Ok(())
    }
}

fn validate_browser_text_target(target: WidgetId, semantics: &Semantics) -> Result<()> {
    if !matches!(semantics.role, Role::TextInput | Role::Input) {
        anyhow::bail!("browser island text target {target} is not a text input");
    }
    if semantics.disabled {
        anyhow::bail!("browser island text target {target} is disabled");
    }
    if semantics.read_only {
        anyhow::bail!("browser island text target {target} is read-only");
    }
    if !semantics
        .actions
        .entries
        .iter()
        .any(|entry| entry.trigger == ActionTrigger::TextChanged)
    {
        anyhow::bail!("browser island text target {target} has no text-change action");
    }
    Ok(())
}

fn lower_browser_island_widget(
    node: Widget,
    portals: Vec<(Option<WidgetId>, Widget)>,
    theme: &Theme,
    runtime: &RuntimeState,
) -> CoreIR {
    let node = compose_browser_island_portals(node, portals);
    let mut env = Env::default();
    env.theme = theme.clone();
    let mut lowering = InternalLoweringCx::new(&env, runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut lowering);
    lowering.ir.set_root(root);
    lowering.ir
}

fn bridge_text_offset(message: &Value, field: &str, text: &str) -> Result<usize> {
    let raw = message
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("browser island text event is missing {field}"))?;
    let offset = usize::try_from(raw)
        .with_context(|| format!("browser island text {field} does not fit usize"))?;
    if offset > text.len() || !text.is_char_boundary(offset) {
        anyhow::bail!("browser island text {field} is not a UTF-8 boundary");
    }
    Ok(offset)
}

struct BrowserIslandBuildOutput<S: GlobalState> {
    node: Widget,
    registry: fission_core::registry::ActionRegistry<S>,
    motion_declarations: Vec<fission_core::MotionDeclaration>,
    video_registrations: Vec<VideoRegistration>,
    web_registrations: Vec<WebRegistration>,
    portals: Vec<(Option<WidgetId>, Widget)>,
}

fn compose_browser_island_portals(
    node: Widget,
    portals: Vec<(Option<WidgetId>, Widget)>,
) -> Widget {
    if portals.is_empty() {
        return node;
    }
    Overlay {
        id: None,
        content: node,
        overlay: ZStack {
            id: None,
            children: portals.into_iter().map(|(_, portal)| portal).collect(),
        }
        .into(),
    }
    .into()
}

/// Runs or initializes a named browser island instance.
///
/// Call this from the island entry function compiled into the route-local WASM
/// artifact. Boot messages reset the named island instance; event messages reuse
/// the existing instance so reducer state is retained across browser events.
pub fn run_browser_island<S, W, F>(id: &str, input: &str, create: F) -> String
where
    S: GlobalState + 'static,
    W: Clone + Into<Widget> + 'static,
    F: FnOnce() -> BrowserIslandApp<S, W>,
{
    let message = parse_bridge_message(input);
    let reset = !message
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "event");

    let result = BROWSER_ISLANDS.with(|instances| {
        let mut instances = instances.borrow_mut();
        if reset || !instances.contains_key(id) {
            instances.insert(id.to_string(), Box::new(create()));
        }
        let island = instances
            .get_mut(id)
            .and_then(|entry| entry.downcast_mut::<BrowserIslandApp<S, W>>())
            .ok_or_else(|| anyhow!("browser island `{id}` has a different concrete type"))?;
        island.handle(input)
    });

    match result {
        Ok(output) => output,
        Err(error) => browser_island_error(id, error),
    }
}

fn parse_bridge_message(input: &str) -> Value {
    serde_json::from_str(input).unwrap_or_else(|_| json!({ "type": "boot" }))
}

fn is_action_event(message: &Value) -> bool {
    message
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "event")
        && message
            .get("binding")
            .and_then(|binding| binding.get("message"))
            .is_some_and(|action| {
                action
                    .get("fission_browser_action")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    || action
                        .get("fission_browser_text_action")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
}

fn is_browser_text_action(message: &Value) -> bool {
    message
        .get("binding")
        .and_then(|binding| binding.get("message"))
        .and_then(|action| action.get("fission_browser_text_action"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn browser_island_error(id: &str, error: anyhow::Error) -> String {
    json!({
        "messages": [
            {
                "type": "error",
                "message": format!("browser island `{id}` failed: {error}"),
                "stack": null
            }
        ],
        "bindings": []
    })
    .to_string()
}

fn hex_decode(value: &str) -> Result<Vec<u8>> {
    if value.len() % 2 != 0 {
        anyhow::bail!("hex payload has odd length");
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_value(bytes[index]).ok_or_else(|| anyhow!("invalid hex payload"))?;
        let low = hex_value(bytes[index + 1]).ok_or_else(|| anyhow!("invalid hex payload"))?;
        out.push((high << 4) | low);
        index += 2;
    }
    Ok(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::op::Color;
    use fission_core::ui::TextInput;
    use fission_core::{
        reduce, reduce_with, Action, ActionInput, ActionScope, ActionScopeId, Button, Effect,
        ReducerContext, RuntimeEffect, StateField, Text,
    };
    use fission_ir::semantics::TextInputType;

    #[derive(Debug, Default, Clone)]
    struct CounterState {
        count: u32,
    }
    impl GlobalState for CounterState {}

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct Increment;
    impl fission_core::Action for Increment {
        fn static_id() -> ActionId {
            ActionId::from_name("fission.site.browser-island.increment")
        }
    }

    fn increment(
        state: &mut CounterState,
        _action: Increment,
        _ctx: &mut ReducerContext<CounterState>,
    ) {
        state.count += 1;
    }

    #[derive(Clone)]
    struct CounterIsland;

    impl From<CounterIsland> for Widget {
        fn from(_component: CounterIsland) -> Widget {
            let (ctx, view) = fission_core::build::current::<CounterState>();
            let action = ctx.bind(Increment, reduce_with!(increment));
            Button {
                child: Some(
                    Text::new(format!("{} clicks", view.state().count))
                        .color(Color::BLACK)
                        .into(),
                ),
                on_press: Some(action),
                ..Default::default()
            }
            .into()
        }
    }

    #[derive(Debug, Default, Clone)]
    struct FieldState {
        field: String,
        value: String,
        observed_node: Option<WidgetId>,
        observed_caret: usize,
        observed_anchor: usize,
    }
    impl GlobalState for FieldState {}

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct UpdateField(String);
    impl Action for UpdateField {
        fn static_id() -> ActionId {
            ActionId::from_name("fission.site.browser-island.update-field")
        }
    }

    fn update_field(
        state: &mut FieldState,
        action: UpdateField,
        ctx: &mut ReducerContext<FieldState>,
    ) {
        let change = ctx
            .input
            .text_change()
            .expect("browser text edit must carry ActionInput");
        state.field = action.0;
        state.value = change.new_text.clone();
        state.observed_node = Some(change.node_id);
        state.observed_caret = change.new_caret;
        state.observed_anchor = change.new_anchor;
    }

    #[derive(Clone)]
    struct FieldIsland;

    impl From<FieldIsland> for Widget {
        fn from(_component: FieldIsland) -> Widget {
            let (ctx, view) = fission_core::build::current::<FieldState>();
            TextInput {
                id: Some(WidgetId::from_u128(901)),
                value: view.state().value.clone(),
                on_input: Some(
                    ctx.bind(UpdateField("smtp_host".into()), reduce_with!(update_field)),
                ),
                ..Default::default()
            }
            .into()
        }
    }

    #[derive(Clone)]
    struct LocalFieldIsland;

    fn update_local_field(
        value: &mut String,
        _action: UpdateField,
        ctx: &mut ReducerContext<String>,
    ) {
        *value = ctx
            .input
            .text_change()
            .expect("local browser text edit must carry ActionInput")
            .new_text
            .clone();
    }

    impl From<LocalFieldIsland> for Widget {
        fn from(_component: LocalFieldIsland) -> Widget {
            let (ctx, _) = fission_core::build::current::<()>();
            let value = StateField::new("BrowserIslandField", "value", String::new());
            TextInput {
                id: Some(WidgetId::from_u128(902)),
                value: value.get(),
                on_input: Some(ctx.bind_local(
                    UpdateField("smtp_host".into()),
                    value,
                    reduce!(update_local_field),
                )),
                ..Default::default()
            }
            .into()
        }
    }

    #[derive(Clone)]
    struct NumberFieldIsland;

    impl From<NumberFieldIsland> for Widget {
        fn from(_component: NumberFieldIsland) -> Widget {
            let (ctx, view) = fission_core::build::current::<FieldState>();
            TextInput {
                id: Some(WidgetId::from_u128(903)),
                value: view.state().value.clone(),
                keyboard_type: TextInputType::Number,
                on_input: Some(
                    ctx.bind(UpdateField("smtp_port".into()), reduce_with!(update_field)),
                ),
                ..Default::default()
            }
            .into()
        }
    }

    #[derive(Clone)]
    struct NonDispatchableFieldIsland {
        read_only: bool,
    }

    #[derive(Debug, Default, Clone)]
    struct ScopedFieldState {
        scope: Option<u128>,
        target: Option<WidgetId>,
        value: String,
    }
    impl GlobalState for ScopedFieldState {}

    fn update_scoped_field(
        state: &mut ScopedFieldState,
        _action: UpdateField,
        ctx: &mut ReducerContext<ScopedFieldState>,
    ) {
        let ActionInput::ScopedRaw {
            scope_id, target, ..
        } = ctx.input
        else {
            panic!("browser text action must retain its enclosing scope");
        };
        state.scope = Some(*scope_id);
        state.target = Some(*target);
        state.value = ctx.input.text_change().unwrap().new_text.clone();
    }

    #[derive(Clone)]
    struct ScopedFieldIsland;

    impl From<ScopedFieldIsland> for Widget {
        fn from(_component: ScopedFieldIsland) -> Widget {
            let (ctx, view) = fission_core::build::current::<ScopedFieldState>();
            ActionScope::new(
                ActionScopeId::from_u128(5150),
                TextInput {
                    id: Some(WidgetId::from_u128(905)),
                    value: view.state().value.clone(),
                    on_input: Some(ctx.bind(
                        UpdateField("scoped".into()),
                        reduce_with!(update_scoped_field),
                    )),
                    ..Default::default()
                },
            )
            .into()
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct QueueUnsupportedEffect;
    impl Action for QueueUnsupportedEffect {
        fn static_id() -> ActionId {
            ActionId::from_name("fission.site.browser-island.queue-unsupported-effect")
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct EffectFinished;
    impl Action for EffectFinished {
        fn static_id() -> ActionId {
            ActionId::from_name("fission.site.browser-island.effect-finished")
        }
    }

    fn effect_finished(
        _state: &mut CounterState,
        _action: EffectFinished,
        _ctx: &mut ReducerContext<CounterState>,
    ) {
    }

    fn queue_unsupported_effect(
        _state: &mut CounterState,
        _action: QueueUnsupportedEffect,
        ctx: &mut ReducerContext<CounterState>,
    ) {
        let on_ok = ctx
            .effects
            .bind(EffectFinished, reduce_with!(effect_finished));
        ctx.effects
            .add(Effect::Runtime(RuntimeEffect::Cancel { req_id: 77 }))
            .on_ok(on_ok);
    }

    #[derive(Clone)]
    struct EffectIsland;

    impl From<EffectIsland> for Widget {
        fn from(_component: EffectIsland) -> Widget {
            let (ctx, _) = fission_core::build::current::<CounterState>();
            Button {
                on_press: Some(ctx.bind(
                    QueueUnsupportedEffect,
                    reduce_with!(queue_unsupported_effect),
                )),
                child: Some(Text::new("Queue effect").into()),
                ..Default::default()
            }
            .into()
        }
    }

    impl From<NonDispatchableFieldIsland> for Widget {
        fn from(component: NonDispatchableFieldIsland) -> Widget {
            let (ctx, view) = fission_core::build::current::<FieldState>();
            TextInput {
                id: Some(WidgetId::from_u128(904)),
                value: view.state().value.clone(),
                read_only: component.read_only,
                enabled: component.read_only,
                on_input: Some(ctx.bind(UpdateField("blocked".into()), reduce_with!(update_field))),
                ..Default::default()
            }
            .into()
        }
    }

    #[test]
    fn browser_island_runs_reducer_and_rerenders_html() {
        let id = format!("counter-{}", std::process::id());
        let boot = run_browser_island(&id, r#"{"type":"boot"}"#, || {
            BrowserIslandApp::new(&id, "counter-mount", CounterState::default(), CounterIsland)
        });
        assert!(boot.contains("0 clicks"));
        assert!(boot.contains("replace_children_html_by_semantics"));

        let action_id = Increment::static_id().as_u128();
        let payload_hex = test_hex_encode(&Increment.encode());
        let event = format!(
            r#"{{"type":"event","sequence":2,"binding":{{"message":{{"fission_browser_action":true,"action_id":"{action_id}","target_node":"1","payload_hex":"{payload_hex}"}}}}}}"#
        );
        let update = run_browser_island(&id, &event, || {
            BrowserIslandApp::new(&id, "counter-mount", CounterState::default(), CounterIsland)
        });
        assert!(update.contains("1 clicks"));
    }

    #[test]
    fn browser_island_dispatches_text_with_static_context_and_utf8_selection() {
        let id = format!("field-{}", std::process::id());
        let boot = run_browser_island(&id, r#"{"type":"boot"}"#, || {
            BrowserIslandApp::new(&id, "field-mount", FieldState::default(), FieldIsland)
        });
        assert!(boot.contains("data-fission-browser-text-action"));

        let event = r#"{
            "type":"event",
            "sequence":2,
            "binding":{
                "event":"input",
                "message":{
                    "fission_browser_text_action":true,
                    "target_node":"901"
                }
            },
            "value":"café",
            "caret":5,
            "anchor":3
        }"#;
        let update = run_browser_island(&id, event, || {
            BrowserIslandApp::new(&id, "field-mount", FieldState::default(), FieldIsland)
        });
        assert!(!update.contains("browser island `"));
        assert!(update.contains("value=\\\"café\\\""));

        BROWSER_ISLANDS.with(|instances| {
            let instances = instances.borrow();
            let island = instances
                .get(&id)
                .and_then(|entry| entry.downcast_ref::<BrowserIslandApp<FieldState, FieldIsland>>())
                .unwrap();
            let state = island.runtime.get_global_state::<FieldState>().unwrap();
            assert_eq!(state.field, "smtp_host");
            assert_eq!(state.value, "café");
            assert_eq!(state.observed_node, Some(WidgetId::from_u128(901)));
            assert_eq!(state.observed_caret, 5);
            assert_eq!(state.observed_anchor, 3);
        });
    }

    #[test]
    fn browser_island_number_input_dispatches_text_and_selection() {
        let id = format!("field-number-{}", std::process::id());
        let event = r#"{
            "type":"event",
            "binding":{"message":{
                "fission_browser_text_action":true,
                "target_node":"903"
            }},
            "value":"2525",
            "caret":4,
            "anchor":4
        }"#;
        let update = run_browser_island(&id, event, || {
            BrowserIslandApp::new(&id, "field-mount", FieldState::default(), NumberFieldIsland)
        });
        assert!(!update.contains("browser island `"));
        BROWSER_ISLANDS.with(|instances| {
            let instances = instances.borrow();
            let island = instances
                .get(&id)
                .and_then(|entry| {
                    entry.downcast_ref::<BrowserIslandApp<FieldState, NumberFieldIsland>>()
                })
                .unwrap();
            let state = island.runtime.get_global_state::<FieldState>().unwrap();
            assert_eq!(state.field, "smtp_port");
            assert_eq!(state.value, "2525");
            assert_eq!(state.observed_caret, 4);
            assert_eq!(state.observed_anchor, 4);
        });
    }

    #[test]
    fn browser_island_rejects_disabled_and_read_only_text_events() {
        for (read_only, expected) in [(true, "read-only"), (false, "disabled")] {
            let id = format!("field-blocked-{read_only}-{}", std::process::id());
            let event = r#"{
                "type":"event",
                "binding":{"message":{
                    "fission_browser_text_action":true,
                    "target_node":"904"
                }},
                "value":"must-not-dispatch",
                "caret":17,
                "anchor":17
            }"#;
            let output = run_browser_island(&id, event, || {
                BrowserIslandApp::new(
                    &id,
                    "field-mount",
                    FieldState::default(),
                    NonDispatchableFieldIsland { read_only },
                )
            });
            assert!(output.contains(expected), "unexpected output: {output}");
        }
    }

    #[test]
    fn browser_island_text_target_validation_requires_text_role_and_change_action() {
        let target = WidgetId::from_u128(42);
        let generic = Semantics {
            role: Role::Generic,
            ..Default::default()
        };
        assert!(validate_browser_text_target(target, &generic)
            .unwrap_err()
            .to_string()
            .contains("not a text input"));

        let text_without_action = Semantics {
            role: Role::TextInput,
            ..Default::default()
        };
        assert!(validate_browser_text_target(target, &text_without_action)
            .unwrap_err()
            .to_string()
            .contains("has no text-change action"));
    }

    #[test]
    fn browser_island_rejects_selection_that_splits_utf8() {
        let id = format!("field-invalid-{}", std::process::id());
        let event = r#"{
            "type":"event",
            "binding":{
                "message":{
                    "fission_browser_text_action":true,
                    "target_node":"901"
                }
            },
            "value":"é",
            "caret":1,
            "anchor":0
        }"#;
        let update = run_browser_island(&id, event, || {
            BrowserIslandApp::new(&id, "field-mount", FieldState::default(), FieldIsland)
        });
        assert!(update.contains("caret is not a UTF-8 boundary"));
    }

    #[test]
    fn browser_island_text_dispatch_preserves_enclosing_action_scope() {
        let id = format!("field-scoped-{}", std::process::id());
        let event = r#"{
            "type":"event",
            "binding":{"message":{
                "fission_browser_text_action":true,
                "target_node":"905"
            }},
            "value":"scoped value",
            "caret":12,
            "anchor":12
        }"#;
        let update = run_browser_island(&id, event, || {
            BrowserIslandApp::new(
                &id,
                "field-mount",
                ScopedFieldState::default(),
                ScopedFieldIsland,
            )
        });
        assert!(!update.contains("browser island `"));
        BROWSER_ISLANDS.with(|instances| {
            let instances = instances.borrow();
            let island = instances
                .get(&id)
                .and_then(|entry| {
                    entry.downcast_ref::<BrowserIslandApp<ScopedFieldState, ScopedFieldIsland>>()
                })
                .unwrap();
            let state = island
                .runtime
                .get_global_state::<ScopedFieldState>()
                .unwrap();
            assert_eq!(state.scope, Some(5150));
            assert_eq!(state.target, Some(WidgetId::from_u128(905)));
            assert_eq!(state.value, "scoped value");
        });
    }

    #[test]
    fn browser_island_sequential_text_edits_keep_field_identity() {
        let id = format!("field-sequential-{}", std::process::id());
        let first = r#"{
            "type":"event",
            "binding":{"message":{
                "fission_browser_text_action":true,
                "target_node":"901"
            }},
            "value":"green",
            "caret":5,
            "anchor":5
        }"#;
        let second = r#"{
            "type":"event",
            "binding":{"message":{
                "fission_browser_text_action":true,
                "target_node":"901"
            }},
            "value":"greenmail",
            "caret":9,
            "anchor":9
        }"#;
        let first_output = run_browser_island(&id, first, || {
            BrowserIslandApp::new(&id, "field-mount", FieldState::default(), FieldIsland)
        });
        assert!(!first_output.contains("browser island `"));
        let second_output = run_browser_island(&id, second, || {
            BrowserIslandApp::new(&id, "field-mount", FieldState::default(), FieldIsland)
        });
        assert!(!second_output.contains("browser island `"));

        BROWSER_ISLANDS.with(|instances| {
            let instances = instances.borrow();
            let island = instances
                .get(&id)
                .and_then(|entry| entry.downcast_ref::<BrowserIslandApp<FieldState, FieldIsland>>())
                .unwrap();
            let state = island.runtime.get_global_state::<FieldState>().unwrap();
            assert_eq!(state.field, "smtp_host");
            assert_eq!(state.value, "greenmail");
            assert_eq!(state.observed_caret, 9);
            assert_eq!(state.observed_anchor, 9);
        });
    }

    #[test]
    fn browser_island_text_updates_bind_local_state() {
        let id = format!("field-local-{}", std::process::id());
        let boot = run_browser_island(&id, r#"{"type":"boot"}"#, || {
            BrowserIslandApp::new(&id, "field-mount", (), LocalFieldIsland)
        });
        assert!(boot.contains("data-fission-browser-text-action"));
        let event = r#"{
            "type":"event",
            "binding":{"message":{
                "fission_browser_text_action":true,
                "target_node":"902"
            }},
            "value":"greenmail",
            "caret":9,
            "anchor":9
        }"#;
        let update = run_browser_island(&id, event, || {
            BrowserIslandApp::new(&id, "field-mount", (), LocalFieldIsland)
        });
        assert!(!update.contains("browser island `"));
        assert!(update.contains("value=\\\"greenmail\\\""));
    }

    #[test]
    fn browser_island_rejects_and_discards_effects_and_callbacks() {
        let mut island = BrowserIslandApp::new(
            "effect-island",
            "effect-mount",
            CounterState::default(),
            EffectIsland,
        );
        let action_id = QueueUnsupportedEffect::static_id().as_u128();
        let payload_hex = test_hex_encode(&QueueUnsupportedEffect.encode());
        let event = format!(
            r#"{{"type":"event","binding":{{"message":{{"fission_browser_action":true,"action_id":"{action_id}","target_node":"1","payload_hex":"{payload_hex}"}}}}}}"#
        );

        let first = island.handle(&event).unwrap_err().to_string();
        assert!(first.contains("discarded 2 effect envelope(s) and completion callback(s)"));
        assert_eq!(island.runtime.discard_pending_effects(), 0);

        let second = island.handle(&event).unwrap_err().to_string();
        assert!(second.contains("discarded 2 effect envelope(s) and completion callback(s)"));
        assert_eq!(island.runtime.discard_pending_effects(), 0);
    }

    fn test_hex_encode(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}
