use crate::{render_ir_to_html_with_styles, CssVariableMap, HtmlRenderOptions, StyleRegistry};
use anyhow::{anyhow, Context, Result};
use fission_core::{
    ActionEnvelope, ActionId, AppState, BuildCtx, Env, LoweringContext, RuntimeState, View, Widget,
};
use fission_ir::NodeId;
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
    S: AppState,
    W: Widget<S> + Clone,
{
    id: String,
    mount_id: String,
    state: S,
    widget: W,
    theme: Theme,
}

impl<S, W> BrowserIslandApp<S, W>
where
    S: AppState,
    W: Widget<S> + Clone,
{
    /// Creates a browser island rooted at a semantic mount point.
    ///
    /// `id` identifies the route-local island instance and is used for
    /// diagnostics and generated stylesheet ids. `mount_id` must match the
    /// semantic identifier rendered by the server page, because the browser
    /// bridge replaces that region with the island's current widget output.
    pub fn new(id: impl Into<String>, mount_id: impl Into<String>, state: S, widget: W) -> Self {
        Self {
            id: id.into(),
            mount_id: mount_id.into(),
            state,
            widget,
            theme: Theme::default(),
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

        let (_node, mut registry) = self.build_node_and_registry();
        registry.dispatch(
            &mut self.state,
            &ActionEnvelope {
                id: ActionId::from_u128(action_id),
                payload,
            },
            NodeId::from_u128(target),
        )?;
        Ok(())
    }

    fn render_bridge_output(&mut self, sequence: u64) -> Result<String> {
        let (node, _registry) = self.build_node_and_registry();
        let runtime = RuntimeState::default();
        let mut env = Env::default();
        env.theme = self.theme.clone();
        let mut lowering = LoweringContext::new(&env, &runtime, None, None);
        let root = node.lower(&mut lowering);
        lowering.ir.set_root(root);

        let mut styles = StyleRegistry::default();
        let rendered = render_ir_to_html_with_styles(
            &lowering.ir,
            &HtmlRenderOptions {
                document_title: self.id.clone(),
                root_class: "fission-browser-island-root".to_string(),
                css_variables: CssVariableMap::from_theme(&self.theme),
                browser_action_bindings: true,
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

    fn build_node_and_registry(
        &self,
    ) -> (
        fission_core::Node,
        fission_core::registry::ActionRegistry<S>,
    ) {
        let runtime = RuntimeState::default();
        let mut env = Env::default();
        env.theme = self.theme.clone();
        let view = View::new(&self.state, &runtime, &env, None);
        let mut ctx = BuildCtx::<S>::new();
        let node = self.widget.clone().build(&mut ctx, &view);
        (node, ctx.registry)
    }
}

/// Runs or initializes a named browser island instance.
///
/// Call this from the island entry function compiled into the route-local WASM
/// artifact. Boot messages reset the named island instance; event messages reuse
/// the existing instance so reducer state is retained across browser events.
pub fn run_browser_island<S, W, F>(id: &str, input: &str, create: F) -> String
where
    S: AppState + 'static,
    W: Widget<S> + Clone + 'static,
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
            .and_then(|action| action.get("fission_browser_action"))
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
    use fission_core::{reduce_with, Action, Button, Node, ReducerContext, Text};

    #[derive(Debug, Default, Clone)]
    struct CounterState {
        count: u32,
    }
    impl AppState for CounterState {}

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

    impl Widget<CounterState> for CounterIsland {
        fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
            let action = ctx.bind(Increment, reduce_with!(increment));
            Button {
                child: Some(Box::new(
                    Text::new(format!("{} clicks", view.state.count))
                        .color(Color::BLACK)
                        .into_node(),
                )),
                on_press: Some(action),
                ..Default::default()
            }
            .into_node()
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
