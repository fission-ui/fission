//! Browser accessibility mirror for the canvas-backed Web shell.
//!
//! The canvas remains the visual renderer. This module projects the same
//! `Semantics` nodes used by native AccessKit hosts into transparent DOM nodes,
//! so browser accessibility APIs and sequential keyboard navigation have a
//! real, production surface rather than depending on test-control metadata.

use fission_core::KeyCode;
use fission_ir::semantics::{Role, TextFieldValidationState};
use fission_ir::Semantics;

#[derive(Debug, Clone, PartialEq)]
struct SemanticDescriptor {
    role: Option<&'static str>,
    accessible_name: Option<String>,
    exposed_text: Option<String>,
    checked: Option<bool>,
    disabled: bool,
    read_only: bool,
    required: bool,
    invalid: bool,
    description: Option<String>,
    live: bool,
    multiline: bool,
    modal: bool,
    min_value: Option<f32>,
    max_value: Option<f32>,
    current_value: Option<f32>,
    value_text: Option<String>,
}

impl SemanticDescriptor {
    fn new(semantics: &Semantics, label: Option<String>, value: Option<String>) -> Self {
        let exposed_text = (semantics.role == Role::Text)
            .then(|| label.clone().or_else(|| value.clone()))
            .flatten();
        let accessible_name = (semantics.role != Role::Text).then_some(label).flatten();
        let (value_text, value_description) = match semantics.role {
            Role::Slider => (value, None),
            Role::Text | Role::TextInput | Role::Input => (None, None),
            _ => (None, value),
        };
        let description = semantics.validation_message.clone().or(value_description);
        Self {
            role: aria_role(semantics.role),
            accessible_name,
            exposed_text,
            checked: semantics.checked,
            disabled: semantics.disabled,
            read_only: semantics.read_only,
            required: semantics.required,
            invalid: semantics.validation_state == TextFieldValidationState::Invalid,
            live: invalid_validation(semantics) && semantics.validation_message.is_some(),
            description,
            multiline: semantics.multiline,
            modal: semantics.role == Role::Dialog && semantics.is_focus_barrier,
            min_value: semantics.min_value,
            max_value: semantics.max_value,
            current_value: semantics.current_value,
            value_text,
        }
    }
}

fn invalid_validation(semantics: &Semantics) -> bool {
    semantics.validation_state == TextFieldValidationState::Invalid
}

fn aria_role(role: Role) -> Option<&'static str> {
    match role {
        Role::Button => Some("button"),
        Role::Link => Some("link"),
        Role::MenuItem => Some("menuitem"),
        Role::Text => None,
        Role::TextInput | Role::Input => Some("textbox"),
        Role::Image => Some("img"),
        Role::Checkbox => Some("checkbox"),
        Role::Radio => Some("radio"),
        Role::Switch => Some("switch"),
        Role::Dialog => Some("dialog"),
        Role::Slider => Some("slider"),
        Role::List => Some("list"),
        Role::ListItem => Some("listitem"),
        Role::Generic => Some("group"),
    }
}

fn web_key(key: &str) -> Option<(KeyCode, Option<String>)> {
    let code = match key {
        " " | "Spacebar" => KeyCode::Space,
        "Enter" => KeyCode::Enter,
        "Escape" | "Esc" => KeyCode::Escape,
        "Backspace" => KeyCode::Backspace,
        "Delete" | "Del" => KeyCode::Delete,
        "Tab" => KeyCode::Tab,
        "ArrowLeft" | "Left" => KeyCode::Left,
        "ArrowRight" | "Right" => KeyCode::Right,
        "ArrowUp" | "Up" => KeyCode::Up,
        "ArrowDown" | "Down" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        printable if printable.chars().count() == 1 => KeyCode::Char(printable.chars().next()?),
        _ => return None,
    };
    let produced_text = matches!(code, KeyCode::Char(_)).then(|| key.to_string());
    Some((code, produced_text))
}

fn bridge_owns_key(code: &KeyCode, focus_barrier_active: bool) -> bool {
    !matches!(code, KeyCode::Tab) || focus_barrier_active
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use fission_core::event::{KeyEvent, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_SUPER};
    use fission_core::{ActionInput, InputEvent, Runtime, TextEditSource};
    use fission_ir::{CoreIR, Op, PaintOp, WidgetId};
    use fission_layout::{LayoutRect, LayoutSnapshot};
    use fission_test_driver::TestEvent;
    use wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{Element, Event, HtmlCanvasElement, HtmlElement, KeyboardEvent};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::platform::web::WindowExtWebSys;
    use winit::window::Window;

    use super::{bridge_owns_key, web_key, SemanticDescriptor, Semantics};
    use crate::BrowserDefaults;

    static NEXT_ROOT_ID: AtomicU32 = AtomicU32::new(1);

    const MANAGED_ATTRIBUTES: &[&str] = &[
        "role",
        "aria-label",
        "aria-checked",
        "aria-disabled",
        "aria-readonly",
        "aria-required",
        "aria-invalid",
        "aria-description",
        "aria-live",
        "aria-atomic",
        "aria-multiline",
        "aria-modal",
        "aria-valuemin",
        "aria-valuemax",
        "aria-valuenow",
        "aria-valuetext",
        "aria-owns",
        "aria-hidden",
        "tabindex",
    ];

    #[derive(Debug)]
    enum QueuedAccessibilityEvent {
        Focus(WidgetId),
        Blur(WidgetId),
        Activate(WidgetId),
        KeyDown {
            target: WidgetId,
            code: fission_core::KeyCode,
            modifiers: u8,
            produced_text: Option<String>,
        },
        KeyUp {
            code: fission_core::KeyCode,
            modifiers: u8,
        },
    }

    struct WebNode {
        element: HtmlElement,
    }

    struct WebImeElement {
        element: HtmlElement,
        aria_hidden: Option<String>,
        aria_multiline: Option<String>,
        marker: Option<String>,
        widget_id: Option<String>,
    }

    struct CanvasAttributes {
        aria_hidden: Option<String>,
        tabindex: Option<String>,
    }

    struct WebAccessibilityRoot {
        canvas: HtmlCanvasElement,
        root: HtmlElement,
        nodes: HashMap<WidgetId, WebNode>,
        dom_order: Vec<WidgetId>,
        id_prefix: String,
        canvas_attributes: CanvasAttributes,
        ime_elements: Vec<WebImeElement>,
        _focus_listener: Closure<dyn FnMut(Event)>,
        _focus_out_listener: Closure<dyn FnMut(Event)>,
        _click_listener: Closure<dyn FnMut(Event)>,
        _key_down_listener: Closure<dyn FnMut(KeyboardEvent)>,
        _key_up_listener: Closure<dyn FnMut(KeyboardEvent)>,
        _ime_focus_out_listeners: Vec<Closure<dyn FnMut(Event)>>,
    }

    impl Drop for WebAccessibilityRoot {
        fn drop(&mut self) {
            restore_attribute(
                &self.canvas,
                "aria-hidden",
                self.canvas_attributes.aria_hidden.as_deref(),
            );
            restore_attribute(
                &self.canvas,
                "tabindex",
                self.canvas_attributes.tabindex.as_deref(),
            );
            let _ = self
                .canvas
                .remove_attribute("data-fission-semantics-mirrored");
            for (ime, listener) in self.ime_elements.iter().zip(&self._ime_focus_out_listeners) {
                let _ = ime.element.remove_event_listener_with_callback(
                    "focusout",
                    listener.as_ref().unchecked_ref(),
                );
                restore_html_attribute(&ime.element, "aria-hidden", ime.aria_hidden.as_deref());
                restore_html_attribute(
                    &ime.element,
                    "aria-multiline",
                    ime.aria_multiline.as_deref(),
                );
                restore_html_attribute(
                    &ime.element,
                    "data-fission-ime-proxy",
                    ime.marker.as_deref(),
                );
                restore_html_attribute(
                    &ime.element,
                    "data-fission-widget-id",
                    ime.widget_id.as_deref(),
                );
            }
            if let Some(parent) = self.root.parent_node() {
                let _ = parent.remove_child(&self.root);
            }
        }
    }

    pub struct WebAccessibilityBridge {
        proxy: EventLoopProxy<TestEvent>,
        browser_defaults: BrowserDefaults,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        syncing_focus: Rc<Cell<bool>>,
        root: Option<WebAccessibilityRoot>,
        mount_failed: bool,
        last_runtime_focus: Option<WidgetId>,
        focus_barrier_active: Rc<Cell<bool>>,
    }

    impl WebAccessibilityBridge {
        pub fn new(proxy: EventLoopProxy<TestEvent>, browser_defaults: BrowserDefaults) -> Self {
            Self {
                proxy,
                browser_defaults,
                queue: Rc::new(RefCell::new(VecDeque::new())),
                syncing_focus: Rc::new(Cell::new(false)),
                root: None,
                mount_failed: false,
                last_runtime_focus: None,
                focus_barrier_active: Rc::new(Cell::new(false)),
            }
        }

        pub fn ensure_adapter(&mut self, _event_loop: &ActiveEventLoop, window: &Window) {
            if self.root.is_some() || self.mount_failed {
                return;
            }
            match self.mount(window) {
                Ok(root) => self.root = Some(root),
                Err(error) => {
                    self.mount_failed = true;
                    crate::web_console::error(&format!(
                        "fission-shell-winit: Web accessibility mount failed: {error}"
                    ));
                }
            }
        }

        pub fn process_window_event(&mut self, _window: &Window, _event: &WindowEvent) {}

        pub fn is_active(&self) -> bool {
            self.root.is_some()
        }

        pub fn update_tree(
            &mut self,
            ir: &CoreIR,
            layout: &LayoutSnapshot,
            runtime: &Runtime,
            scale_factor: f64,
        ) {
            let Some(root) = self.root.as_mut() else {
                return;
            };
            sync_root_geometry(root, scale_factor);

            let ordered_ids = accessible_ids_in_tree_order(ir);
            let retained_ids = ordered_ids.iter().copied().collect::<HashSet<_>>();
            let stale_ids = root
                .nodes
                .keys()
                .filter(|id| !retained_ids.contains(id))
                .copied()
                .collect::<Vec<_>>();
            for stale_id in stale_ids {
                if let Some(stale) = root.nodes.remove(&stale_id) {
                    if let Some(parent) = stale.element.parent_node() {
                        let _ = parent.remove_child(&stale.element);
                    }
                }
            }

            let active_barrier = fission_core::hit_test::topmost_focus_barrier(ir);
            self.focus_barrier_active.set(active_barrier.is_some());
            let active_ids = ordered_ids
                .iter()
                .copied()
                .filter(|id| {
                    active_barrier.map_or(true, |barrier| {
                        fission_core::hit_test::is_descendant_or_self(ir, *id, barrier)
                    })
                })
                .collect::<HashSet<_>>();

            for id in &ordered_ids {
                let Some(node) = ir.nodes.get(id) else {
                    continue;
                };
                if !root.nodes.contains_key(id) {
                    match create_web_node(&root.root, &root.id_prefix, *id) {
                        Ok(node) => {
                            root.nodes.insert(*id, node);
                        }
                        Err(error) => {
                            crate::web_console::error(&format!(
                                "fission-shell-winit: failed to create Web accessibility node: {error}"
                            ));
                            continue;
                        }
                    }
                }
                let web_node = root
                    .nodes
                    .get(id)
                    .expect("created Web accessibility node remains retained");
                let active = active_ids.contains(id);
                match &node.op {
                    Op::Semantics(semantics) => {
                        let label = semantics.label.clone().or_else(|| {
                            semantics_consumes_descendant_text(semantics)
                                .then(|| collect_descendant_text(ir, *id))
                                .flatten()
                        });
                        let value = semantic_value(runtime, *id, semantics);
                        let modal = semantics.role == fission_ir::Role::Dialog
                            && active_barrier.is_some_and(|barrier| {
                                fission_core::hit_test::is_descendant_or_self(ir, *id, barrier)
                            });
                        apply_semantics(&web_node.element, semantics, label, value, active, modal);
                    }
                    Op::Paint(PaintOp::DrawText { text, .. }) if !text.is_empty() => {
                        apply_text(&web_node.element, text, active);
                    }
                    Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                        apply_text(&web_node.element, &text, active);
                    }
                    _ => continue,
                }
                apply_node_geometry(
                    &web_node.element,
                    root,
                    crate::visual_rect_for_node(ir, layout, &runtime.runtime_state.scroll, *id),
                    scale_factor,
                );
            }

            // Keep every retained element as a direct child. `aria-owns`
            // projects the semantic hierarchy without reparents on ordinary
            // frames, which would otherwise disrupt browser focus.
            let mut owned_children = HashMap::<Option<WidgetId>, Vec<WidgetId>>::new();
            for id in ordered_ids.iter().filter(|id| active_ids.contains(id)) {
                let parent = nearest_active_accessible_parent(ir, *id, &active_ids);
                owned_children.entry(parent).or_default().push(*id);
            }
            set_owned_nodes(&root.root, owned_children.get(&None), &root.nodes);
            for id in &ordered_ids {
                if let Some(web_node) = root.nodes.get(id) {
                    set_owned_nodes(
                        &web_node.element,
                        owned_children.get(&Some(*id)),
                        &root.nodes,
                    );
                }
            }
            if root.dom_order != ordered_ids {
                for id in &ordered_ids {
                    let Some(web_node) = root.nodes.get(id) else {
                        continue;
                    };
                    let _ = root.root.append_child(&web_node.element);
                }
                root.dom_order.clone_from(&ordered_ids);
            }
            self.sync_dom_focus(runtime, ir);
        }

        pub fn drain_events(
            &mut self,
            runtime: &mut Runtime,
            ir: Option<&CoreIR>,
            layout: Option<&LayoutSnapshot>,
        ) -> bool {
            let Some(ir) = ir else {
                self.queue.borrow_mut().clear();
                return false;
            };
            let Some(layout) = layout else {
                self.queue.borrow_mut().clear();
                return false;
            };
            let mut changed = false;
            loop {
                let event = self.queue.borrow_mut().pop_front();
                let Some(event) = event else {
                    break;
                };
                changed |= match event {
                    QueuedAccessibilityEvent::Focus(target) => {
                        self.syncing_focus.set(true);
                        let changed = focus_semantic_node(runtime, ir, target);
                        if editable_text_input(ir, runtime.runtime_state.interaction.focused)
                            .is_some()
                        {
                            // Focusing through the DOM/assistive-technology
                            // mirror must populate the native IME proxy just
                            // like pointer and core-keyboard focus paths do.
                            runtime.update_focused_ime_state(ir, layout);
                        }
                        self.syncing_focus.set(false);
                        changed
                    }
                    QueuedAccessibilityEvent::Blur(target) => {
                        if runtime.runtime_state.interaction.focused != Some(target)
                            || self.dom_focus_matches_widget(target)
                        {
                            false
                        } else {
                            let prior_dom_focus = self.active_html_element();
                            self.syncing_focus.set(true);
                            let changed = runtime
                                .set_focused_widget(ir, None, TextEditSource::Accessibility)
                                .unwrap_or(false);
                            self.restore_focus_displaced_by_ime_shutdown(prior_dom_focus);
                            self.syncing_focus.set(false);
                            changed
                        }
                    }
                    QueuedAccessibilityEvent::Activate(target) => {
                        activate_semantic_node(runtime, ir, target)
                    }
                    QueuedAccessibilityEvent::KeyDown {
                        target,
                        code,
                        modifiers,
                        produced_text,
                    } => {
                        self.syncing_focus.set(true);
                        let _ = focus_semantic_node(runtime, ir, target);
                        self.syncing_focus.set(false);
                        let event = produced_text.map_or_else(
                            || KeyEvent::Down {
                                key_code: code.clone(),
                                modifiers,
                            },
                            |text| KeyEvent::DownWithText {
                                key_code: code.clone(),
                                modifiers,
                                text,
                            },
                        );
                        runtime
                            .handle_input(InputEvent::Keyboard(event), ir, layout)
                            .is_ok()
                    }
                    QueuedAccessibilityEvent::KeyUp { code, modifiers } => runtime
                        .handle_input(
                            InputEvent::Keyboard(KeyEvent::Up {
                                key_code: code,
                                modifiers,
                            }),
                            ir,
                            layout,
                        )
                        .is_ok(),
                };
            }
            self.sync_dom_focus(runtime, ir);
            changed
        }

        fn mount(&self, window: &Window) -> Result<WebAccessibilityRoot, String> {
            let canvas = window
                .canvas()
                .ok_or_else(|| "winit Web window did not expose its canvas".to_string())?;
            let browser_window =
                web_sys::window().ok_or_else(|| "browser window is unavailable".to_string())?;
            let document = browser_window
                .document()
                .ok_or_else(|| "browser document is unavailable".to_string())?;
            let root = document
                .create_element("div")
                .map_err(crate::js_error_to_string)?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "browser created a non-HTML accessibility root".to_string())?;
            let root_number = NEXT_ROOT_ID.fetch_add(1, Ordering::Relaxed);
            let id_prefix = format!("fission-a11y-{root_number}");
            root.set_attribute("id", &id_prefix)
                .map_err(crate::js_error_to_string)?;
            root.set_attribute("data-fission-accessibility-root", "")
                .map_err(crate::js_error_to_string)?;
            root.set_attribute("role", "application")
                .map_err(crate::js_error_to_string)?;
            let title = document.title();
            let canvas_label = canvas.get_attribute("aria-label");
            if let Some(label) = canvas_label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
            {
                root.set_attribute("aria-label", label)
                    .map_err(crate::js_error_to_string)?;
            } else if !title.trim().is_empty() {
                root.set_attribute("aria-label", &title)
                    .map_err(crate::js_error_to_string)?;
            }
            root.set_attribute(
                "style",
                "position:fixed;left:0;top:0;width:0;height:0;overflow:hidden;pointer-events:none;background:transparent;z-index:2147483647;",
            )
            .map_err(crate::js_error_to_string)?;

            let style = document
                .create_element("style")
                .map_err(crate::js_error_to_string)?;
            style
                .set_attribute("aria-hidden", "true")
                .map_err(crate::js_error_to_string)?;
            style.set_text_content(Some(
                "[data-fission-a11y-node]{position:fixed!important;box-sizing:border-box!important;margin:0!important;padding:0!important;border:0!important;background:transparent!important;color:transparent!important;text-shadow:none!important;overflow:hidden!important;white-space:nowrap!important;pointer-events:none!important;-webkit-appearance:none!important;appearance:none!important;}[data-fission-a11y-node]:focus-visible{outline:3px solid Highlight!important;outline-offset:2px!important;}",
            ));
            root.append_child(&style)
                .map_err(crate::js_error_to_string)?;

            let parent = canvas
                .parent_element()
                .or_else(|| document.body().map(Into::into))
                .ok_or_else(|| "Web canvas has no mount parent".to_string())?;
            parent
                .append_child(&root)
                .map_err(crate::js_error_to_string)?;

            let canvas_attributes = CanvasAttributes {
                aria_hidden: canvas.get_attribute("aria-hidden"),
                tabindex: canvas.get_attribute("tabindex"),
            };
            let ime_elements = find_winit_ime_elements(&canvas);
            for ime in &ime_elements {
                let _ = ime.element.set_attribute("aria-hidden", "true");
                let _ = ime.element.set_attribute("data-fission-ime-proxy", "");
            }
            if let Some(canvas_element) = canvas.dyn_ref::<HtmlElement>() {
                let _ = canvas_element.blur();
            }
            canvas
                .set_attribute("aria-hidden", "true")
                .map_err(crate::js_error_to_string)?;
            canvas
                .set_attribute("tabindex", "-1")
                .map_err(crate::js_error_to_string)?;
            canvas
                .set_attribute("data-fission-semantics-mirrored", "")
                .map_err(crate::js_error_to_string)?;

            let focus_listener = make_focus_listener(
                &root,
                self.queue.clone(),
                self.proxy.clone(),
                self.syncing_focus.clone(),
            )?;
            let focus_out_listener = make_focus_out_listener(
                &root,
                self.queue.clone(),
                self.proxy.clone(),
                self.syncing_focus.clone(),
            )?;
            let click_listener =
                make_click_listener(&root, self.queue.clone(), self.proxy.clone())?;
            let key_down_listener = make_key_down_listener(
                &root,
                self.queue.clone(),
                self.proxy.clone(),
                self.browser_defaults,
                self.focus_barrier_active.clone(),
            )?;
            let key_up_listener = make_key_up_listener(
                &root,
                self.queue.clone(),
                self.proxy.clone(),
                self.browser_defaults,
                self.focus_barrier_active.clone(),
            )?;
            let ime_focus_out_listeners = ime_elements
                .iter()
                .map(|ime| {
                    make_ime_focus_out_listener(
                        &ime.element,
                        self.queue.clone(),
                        self.proxy.clone(),
                        self.syncing_focus.clone(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(WebAccessibilityRoot {
                canvas,
                root,
                nodes: HashMap::new(),
                dom_order: Vec::new(),
                id_prefix,
                canvas_attributes,
                ime_elements,
                _focus_listener: focus_listener,
                _focus_out_listener: focus_out_listener,
                _click_listener: click_listener,
                _key_down_listener: key_down_listener,
                _key_up_listener: key_up_listener,
                _ime_focus_out_listeners: ime_focus_out_listeners,
            })
        }

        fn sync_dom_focus(&mut self, runtime: &Runtime, ir: &CoreIR) {
            let focused = runtime.runtime_state.interaction.focused;
            let focused_text_input = editable_text_input(ir, focused);
            if let Some(root) = self.root.as_ref() {
                let native_ime_owns_focus = sync_ime_visibility(root, focused_text_input);
                if native_ime_owns_focus {
                    if let Some(element) = focused.and_then(|id| root.nodes.get(&id)) {
                        let _ = element.element.remove_attribute("tabindex");
                        let _ = element.element.set_attribute("aria-hidden", "true");
                    }
                    // Winit's native textarea/password proxy owns composition,
                    // selection and mobile-keyboard input while a TextInput is
                    // focused. Never steal that focus back to the mirror.
                    self.last_runtime_focus = focused;
                    return;
                }
                if focused_text_input.is_some() && root.ime_elements.is_empty() {
                    // Without a uniquely identified native proxy, do not steal
                    // focus from whichever private element winit may be using
                    // for composition. The mirror remains exposed to AX; a
                    // per-window upstream proxy marker can make this handoff
                    // fully observable in a future winit release.
                    self.last_runtime_focus = focused;
                    return;
                }
            }
            let Some(element) = focused
                .and_then(|id| self.root.as_ref()?.nodes.get(&id))
                .map(|node| node.element.clone())
            else {
                self.last_runtime_focus = focused;
                let root = self.root.as_ref();
                let active_managed = root
                    .and_then(|root| root.root.owner_document())
                    .and_then(|document| document.active_element())
                    .filter(|element| {
                        element.has_attribute("data-fission-a11y-node")
                            || element.has_attribute("data-fission-link-proxy")
                            || root.is_some_and(|root| {
                                element.is_same_node(Some(root.canvas.unchecked_ref()))
                            })
                    });
                if let Some(element) = active_managed
                    .as_ref()
                    .and_then(|element| element.dyn_ref::<HtmlElement>())
                {
                    // Runtime focus is authoritative. Clear only a stale
                    // mirror/link focus or this bridge's exact canvas, never
                    // an unrelated browser control.
                    self.syncing_focus.set(true);
                    let _ = element.blur();
                    self.syncing_focus.set(false);
                }
                return;
            };
            let dom_has_focus = element
                .owner_document()
                .and_then(|document| document.active_element())
                .is_some_and(|active| active.is_same_node(Some(&element)));
            if focused == self.last_runtime_focus && dom_has_focus {
                return;
            }
            // Reordering retained DOM nodes can make Chromium drop focus even
            // though Fission's runtime focus did not change. Reassert the one
            // authoritative runtime target whenever the browser diverges.
            self.last_runtime_focus = focused;
            self.syncing_focus.set(true);
            let _ = element.focus();
            self.syncing_focus.set(false);
        }

        fn dom_focus_matches_widget(&self, id: WidgetId) -> bool {
            self.active_html_element().is_some_and(|element| {
                element.get_attribute("data-fission-widget-id") == Some(id.as_u128().to_string())
            })
        }

        fn active_html_element(&self) -> Option<HtmlElement> {
            self.root
                .as_ref()
                .and_then(|root| root.root.owner_document())
                .and_then(|document| document.active_element())
                .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        }

        fn restore_focus_displaced_by_ime_shutdown(&self, prior: Option<HtmlElement>) {
            let Some(root) = self.root.as_ref() else {
                return;
            };
            let canvas: &HtmlElement = root.canvas.unchecked_ref();
            let canvas_has_focus = || {
                root.root
                    .owner_document()
                    .and_then(|document| document.active_element())
                    .is_some_and(|active| active.is_same_node(Some(canvas)))
            };
            if !canvas_has_focus() {
                return;
            }
            if let Some(prior) = prior
                .filter(|element| element.is_connected() && !element.is_same_node(Some(canvas)))
            {
                let _ = prior.focus();
            }
            if canvas_has_focus() {
                let _ = canvas.blur();
            }
        }
    }

    fn make_focus_listener(
        root: &HtmlElement,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        proxy: EventLoopProxy<TestEvent>,
        syncing_focus: Rc<Cell<bool>>,
    ) -> Result<Closure<dyn FnMut(Event)>, String> {
        let listener = Closure::wrap(Box::new(move |event: Event| {
            if syncing_focus.get() {
                return;
            }
            if let Some(target) = event_widget_id(&event) {
                queue
                    .borrow_mut()
                    .push_back(QueuedAccessibilityEvent::Focus(target));
                let _ = proxy.send_event(TestEvent::Wake);
            }
        }) as Box<dyn FnMut(_)>);
        root.add_event_listener_with_callback("focusin", listener.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        Ok(listener)
    }

    fn find_winit_ime_elements(canvas: &HtmlCanvasElement) -> Vec<WebImeElement> {
        let Some(body) = canvas.owner_document().and_then(|document| document.body()) else {
            return Vec::new();
        };
        let mut matches = Vec::new();
        let mut current = body.first_element_child();
        while let Some(element) = current {
            current = element.next_element_sibling();
            let Ok(element) = element.dyn_into::<HtmlElement>() else {
                continue;
            };
            if is_winit_ime_element(&element, "TEXTAREA", None)
                || is_winit_ime_element(&element, "INPUT", Some("password"))
            {
                matches.push(element);
            }
        }
        // Fission-winit creates exactly one consecutive textarea/password pair
        // for each Web window. Refuse to alter host-page fields when that exact
        // private proxy shape is not unique.
        if matches.len() != 2
            || !is_winit_ime_element(&matches[0], "TEXTAREA", None)
            || !is_winit_ime_element(&matches[1], "INPUT", Some("password"))
            || matches[0]
                .next_element_sibling()
                .is_none_or(|next| !next.is_same_node(Some(&matches[1])))
        {
            return Vec::new();
        }
        matches
            .into_iter()
            .map(|element| WebImeElement {
                aria_hidden: element.get_attribute("aria-hidden"),
                aria_multiline: element.get_attribute("aria-multiline"),
                marker: element.get_attribute("data-fission-ime-proxy"),
                widget_id: element.get_attribute("data-fission-widget-id"),
                element,
            })
            .collect()
    }

    fn is_winit_ime_element(
        element: &HtmlElement,
        expected_tag: &str,
        expected_type: Option<&str>,
    ) -> bool {
        if element.tag_name() != expected_tag
            || expected_type.is_some_and(|value| {
                element
                    .get_attribute("type")
                    .is_none_or(|actual| !actual.eq_ignore_ascii_case(value))
            })
            || element.tab_index() != -1
        {
            return false;
        }
        let style = element.style();
        [
            ("position", "fixed"),
            ("width", "1px"),
            ("height", "1px"),
            ("opacity", "0"),
            ("pointer-events", "none"),
        ]
        .into_iter()
        .all(|(name, expected)| {
            style
                .get_property_value(name)
                .is_ok_and(|actual| actual == expected)
        })
    }

    fn sync_ime_visibility(
        root: &WebAccessibilityRoot,
        focused_text_input: Option<(WidgetId, bool)>,
    ) -> bool {
        let active = root
            .root
            .owner_document()
            .and_then(|document| document.active_element());
        let mut active_proxy = false;
        for ime in &root.ime_elements {
            let owns_focus = active
                .as_ref()
                .is_some_and(|active| active.is_same_node(Some(&ime.element)));
            if let Some((id, _)) = focused_text_input {
                let _ = ime
                    .element
                    .set_attribute("data-fission-widget-id", &id.as_u128().to_string());
            } else {
                let _ = ime.element.remove_attribute("data-fission-widget-id");
            }
            if let Some((_, multiline)) = focused_text_input.filter(|_| owns_focus) {
                active_proxy = true;
                let _ = ime.element.remove_attribute("aria-hidden");
                let _ = ime
                    .element
                    .set_attribute("aria-multiline", bool_string(multiline));
            } else {
                let _ = ime.element.set_attribute("aria-hidden", "true");
                let _ = ime.element.remove_attribute("aria-multiline");
            }
        }
        active_proxy
    }

    fn make_ime_focus_out_listener(
        element: &HtmlElement,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        proxy: EventLoopProxy<TestEvent>,
        syncing_focus: Rc<Cell<bool>>,
    ) -> Result<Closure<dyn FnMut(Event)>, String> {
        let listener = Closure::wrap(Box::new(move |event: Event| {
            if syncing_focus.get() {
                return;
            }
            if let Some(target) = event_widget_id(&event) {
                queue
                    .borrow_mut()
                    .push_back(QueuedAccessibilityEvent::Blur(target));
                let _ = proxy.send_event(TestEvent::Wake);
            }
        }) as Box<dyn FnMut(_)>);
        element
            .add_event_listener_with_callback("focusout", listener.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        Ok(listener)
    }

    fn make_focus_out_listener(
        root: &HtmlElement,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        proxy: EventLoopProxy<TestEvent>,
        syncing_focus: Rc<Cell<bool>>,
    ) -> Result<Closure<dyn FnMut(Event)>, String> {
        let listener = Closure::wrap(Box::new(move |event: Event| {
            if syncing_focus.get() {
                return;
            }
            if let Some(target) = event_widget_id(&event) {
                queue
                    .borrow_mut()
                    .push_back(QueuedAccessibilityEvent::Blur(target));
                let _ = proxy.send_event(TestEvent::Wake);
            }
        }) as Box<dyn FnMut(_)>);
        root.add_event_listener_with_callback("focusout", listener.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        Ok(listener)
    }

    fn make_click_listener(
        root: &HtmlElement,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        proxy: EventLoopProxy<TestEvent>,
    ) -> Result<Closure<dyn FnMut(Event)>, String> {
        let listener = Closure::wrap(Box::new(move |event: Event| {
            let Some(target) = event_widget_id(&event) else {
                return;
            };
            event.prevent_default();
            let mut queue = queue.borrow_mut();
            queue.push_back(QueuedAccessibilityEvent::Focus(target));
            queue.push_back(QueuedAccessibilityEvent::Activate(target));
            drop(queue);
            let _ = proxy.send_event(TestEvent::Wake);
        }) as Box<dyn FnMut(_)>);
        root.add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        Ok(listener)
    }

    fn make_key_down_listener(
        root: &HtmlElement,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        proxy: EventLoopProxy<TestEvent>,
        browser_defaults: BrowserDefaults,
        focus_barrier_active: Rc<Cell<bool>>,
    ) -> Result<Closure<dyn FnMut(KeyboardEvent)>, String> {
        let listener = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let Some(target) = keyboard_event_widget_id(&event) else {
                return;
            };
            let Some((code, produced_text)) = web_key(&event.key()) else {
                return;
            };
            if !bridge_owns_key(&code, focus_barrier_active.get()) {
                // The retained DOM order is the production accessibility
                // order. Outside a modal focus barrier, browser Tab traversal
                // must be able to enter and leave the canvas application.
                return;
            }
            if matches!(code, fission_core::KeyCode::Tab)
                || (!browser_defaults.contains(BrowserDefaults::KEYBOARD)
                    && !is_browser_clipboard_shortcut(&event))
            {
                event.prevent_default();
            }
            let modifiers = modifiers(&event);
            let mut queue = queue.borrow_mut();
            queue.push_back(QueuedAccessibilityEvent::Focus(target));
            queue.push_back(QueuedAccessibilityEvent::KeyDown {
                target,
                code,
                modifiers,
                produced_text,
            });
            drop(queue);
            let _ = proxy.send_event(TestEvent::Wake);
        }) as Box<dyn FnMut(_)>);
        root.add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        Ok(listener)
    }

    fn make_key_up_listener(
        root: &HtmlElement,
        queue: Rc<RefCell<VecDeque<QueuedAccessibilityEvent>>>,
        proxy: EventLoopProxy<TestEvent>,
        browser_defaults: BrowserDefaults,
        focus_barrier_active: Rc<Cell<bool>>,
    ) -> Result<Closure<dyn FnMut(KeyboardEvent)>, String> {
        let listener = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let Some(_target) = keyboard_event_widget_id(&event) else {
                return;
            };
            let Some((code, _)) = web_key(&event.key()) else {
                return;
            };
            if !bridge_owns_key(&code, focus_barrier_active.get()) {
                return;
            }
            if matches!(code, fission_core::KeyCode::Tab)
                || (!browser_defaults.contains(BrowserDefaults::KEYBOARD)
                    && !is_browser_clipboard_shortcut(&event))
            {
                event.prevent_default();
            }
            queue
                .borrow_mut()
                .push_back(QueuedAccessibilityEvent::KeyUp {
                    code,
                    modifiers: modifiers(&event),
                });
            let _ = proxy.send_event(TestEvent::Wake);
        }) as Box<dyn FnMut(_)>);
        root.add_event_listener_with_callback("keyup", listener.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        Ok(listener)
    }

    fn event_widget_id(event: &Event) -> Option<WidgetId> {
        let target = event.target()?.dyn_into::<Element>().ok()?;
        widget_id_from_element(target)
    }

    fn keyboard_event_widget_id(event: &KeyboardEvent) -> Option<WidgetId> {
        let target = event.target()?.dyn_into::<Element>().ok()?;
        widget_id_from_element(target)
    }

    fn widget_id_from_element(mut element: Element) -> Option<WidgetId> {
        loop {
            if let Some(raw) = element.get_attribute("data-fission-widget-id") {
                return raw.parse::<u128>().ok().map(WidgetId::from_u128);
            }
            element = element.parent_element()?;
        }
    }

    fn modifiers(event: &KeyboardEvent) -> u8 {
        let mut modifiers = 0;
        if event.shift_key() {
            modifiers |= MOD_SHIFT;
        }
        if event.alt_key() {
            modifiers |= MOD_ALT;
        }
        if event.ctrl_key() {
            modifiers |= MOD_CTRL;
        }
        if event.meta_key() {
            modifiers |= MOD_SUPER;
        }
        modifiers
    }

    fn is_browser_clipboard_shortcut(event: &KeyboardEvent) -> bool {
        if !(event.ctrl_key() || event.meta_key()) || event.alt_key() {
            return false;
        }
        matches!(
            event.key().to_ascii_lowercase().as_str(),
            "a" | "c" | "v" | "x" | "y" | "z"
        )
    }

    fn accessible_ids_in_tree_order(ir: &CoreIR) -> Vec<WidgetId> {
        let mut ids = Vec::new();
        if let Some(root) = ir.root {
            collect_accessible_ids(ir, root, &mut ids, false);
        }
        ids
    }

    fn collect_accessible_ids(
        ir: &CoreIR,
        id: WidgetId,
        ids: &mut Vec<WidgetId>,
        text_consumed_by_ancestor: bool,
    ) {
        let Some(node) = ir.nodes.get(&id) else {
            return;
        };
        let mut text_consumed = text_consumed_by_ancestor;
        match &node.op {
            Op::Semantics(semantics) => {
                if include_semantics(semantics) {
                    ids.push(id);
                }
                text_consumed |= semantics_consumes_descendant_text(semantics);
            }
            Op::Paint(PaintOp::DrawText { text, .. })
                if !text_consumed_by_ancestor && !text.is_empty() =>
            {
                ids.push(id);
            }
            Op::Paint(PaintOp::DrawRichText { runs, .. })
                if !text_consumed_by_ancestor && runs.iter().any(|run| !run.text.is_empty()) =>
            {
                ids.push(id);
            }
            _ => {}
        }
        for child in &node.children {
            collect_accessible_ids(ir, *child, ids, text_consumed);
        }
    }

    fn semantics_consumes_descendant_text(semantics: &Semantics) -> bool {
        semantics.focusable
            || matches!(
                semantics.role,
                fission_ir::Role::Button
                    | fission_ir::Role::Link
                    | fission_ir::Role::MenuItem
                    | fission_ir::Role::Text
                    | fission_ir::Role::TextInput
                    | fission_ir::Role::Input
                    | fission_ir::Role::Image
                    | fission_ir::Role::Checkbox
                    | fission_ir::Role::Radio
                    | fission_ir::Role::Switch
                    | fission_ir::Role::Slider
            )
    }

    fn include_semantics(semantics: &Semantics) -> bool {
        semantics.role != fission_ir::Role::Generic
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

    fn nearest_active_accessible_parent(
        ir: &CoreIR,
        id: WidgetId,
        active_ids: &HashSet<WidgetId>,
    ) -> Option<WidgetId> {
        let mut current = ir.nodes.get(&id).and_then(|node| node.parent);
        while let Some(parent) = current {
            if active_ids.contains(&parent) {
                return Some(parent);
            }
            current = ir.nodes.get(&parent).and_then(|node| node.parent);
        }
        None
    }

    fn create_web_node(
        root: &HtmlElement,
        id_prefix: &str,
        id: WidgetId,
    ) -> Result<WebNode, String> {
        let document = root
            .owner_document()
            .ok_or_else(|| "accessibility root has no owner document".to_string())?;
        let element = document
            .create_element("div")
            .map_err(crate::js_error_to_string)?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "browser created a non-HTML semantic node".to_string())?;
        element
            .set_attribute("id", &format!("{id_prefix}-{:032x}", id.as_u128()))
            .map_err(crate::js_error_to_string)?;
        element
            .set_attribute("data-fission-a11y-node", "")
            .map_err(crate::js_error_to_string)?;
        element
            .set_attribute("data-fission-widget-id", &id.as_u128().to_string())
            .map_err(crate::js_error_to_string)?;
        root.append_child(&element)
            .map_err(crate::js_error_to_string)?;
        Ok(WebNode { element })
    }

    fn apply_semantics(
        element: &HtmlElement,
        semantics: &Semantics,
        label: Option<String>,
        value: Option<String>,
        active: bool,
        modal: bool,
    ) {
        for attribute in MANAGED_ATTRIBUTES {
            let _ = element.remove_attribute(attribute);
        }
        let _ = element.remove_attribute("data-fission-a11y-text");
        let mut descriptor = SemanticDescriptor::new(semantics, label, value);
        descriptor.modal = modal;
        set_optional_attribute(element, "role", descriptor.role);
        set_optional_attribute(element, "aria-label", descriptor.accessible_name.as_deref());
        set_optional_attribute(element, "aria-checked", descriptor.checked.map(bool_string));
        set_boolean_attribute(element, "aria-disabled", descriptor.disabled);
        set_boolean_attribute(element, "aria-readonly", descriptor.read_only);
        set_boolean_attribute(element, "aria-required", descriptor.required);
        set_boolean_attribute(element, "aria-invalid", descriptor.invalid);
        set_optional_attribute(
            element,
            "aria-description",
            descriptor.description.as_deref(),
        );
        if descriptor.live {
            let _ = element.set_attribute("aria-live", "polite");
            let _ = element.set_attribute("aria-atomic", "true");
        }
        set_boolean_attribute(element, "aria-multiline", descriptor.multiline);
        set_boolean_attribute(element, "aria-modal", descriptor.modal);
        set_optional_attribute(
            element,
            "aria-valuemin",
            descriptor.min_value.as_ref().map(number_string).as_deref(),
        );
        set_optional_attribute(
            element,
            "aria-valuemax",
            descriptor.max_value.as_ref().map(number_string).as_deref(),
        );
        set_optional_attribute(
            element,
            "aria-valuenow",
            descriptor
                .current_value
                .as_ref()
                .map(number_string)
                .as_deref(),
        );
        set_optional_attribute(element, "aria-valuetext", descriptor.value_text.as_deref());
        if let Some(identifier) = semantics.identifier.as_deref() {
            let _ = element.set_attribute("data-fission-semantic-id", identifier);
        } else {
            let _ = element.remove_attribute("data-fission-semantic-id");
        }
        element.set_text_content(descriptor.exposed_text.as_deref());

        if active {
            if semantics.focusable && !semantics.disabled {
                let _ = element.set_attribute("tabindex", "0");
            }
        } else {
            let _ = element.set_attribute("aria-hidden", "true");
        }
    }

    fn apply_text(element: &HtmlElement, text: &str, active: bool) {
        for attribute in MANAGED_ATTRIBUTES {
            let _ = element.remove_attribute(attribute);
        }
        let _ = element.remove_attribute("data-fission-semantic-id");
        let _ = element.set_attribute("data-fission-a11y-text", "");
        element.set_text_content(Some(text));
        if !active {
            let _ = element.set_attribute("aria-hidden", "true");
        }
    }

    fn set_owned_nodes(
        element: &HtmlElement,
        owned: Option<&Vec<WidgetId>>,
        nodes: &HashMap<WidgetId, WebNode>,
    ) {
        let owned = owned
            .into_iter()
            .flatten()
            .filter_map(|id| nodes.get(id))
            .filter_map(|node| node.element.get_attribute("id"))
            .collect::<Vec<_>>()
            .join(" ");
        if owned.is_empty() {
            let _ = element.remove_attribute("aria-owns");
        } else {
            let _ = element.set_attribute("aria-owns", &owned);
        }
    }

    fn sync_root_geometry(root: &WebAccessibilityRoot, scale_factor: f64) {
        let rect = root.canvas.get_bounding_client_rect();
        let style = format!(
            "position:fixed;left:{}px;top:{}px;width:{}px;height:{}px;overflow:hidden;pointer-events:none;background:transparent;z-index:2147483647;",
            rect.x(),
            rect.y(),
            rect.width().max(0.0),
            rect.height().max(0.0)
        );
        let _ = root.root.set_attribute("style", &style);
        let _ = scale_factor;
    }

    fn apply_node_geometry(
        element: &HtmlElement,
        root: &WebAccessibilityRoot,
        rect: Option<LayoutRect>,
        scale_factor: f64,
    ) {
        let canvas_rect = root.canvas.get_bounding_client_rect();
        let logical_width = (root.canvas.width() as f64 / scale_factor.max(0.01)).max(1.0);
        let logical_height = (root.canvas.height() as f64 / scale_factor.max(0.01)).max(1.0);
        let scale_x = canvas_rect.width() / logical_width;
        let scale_y = canvas_rect.height() / logical_height;
        let (left, top, width, height) = rect.map_or((-10_000.0, -10_000.0, 1.0, 1.0), |rect| {
            (
                canvas_rect.x() + rect.x() as f64 * scale_x,
                canvas_rect.y() + rect.y() as f64 * scale_y,
                (rect.width() as f64 * scale_x).max(0.0),
                (rect.height() as f64 * scale_y).max(0.0),
            )
        });
        let style = format!(
            "position:fixed!important;left:{left}px!important;top:{top}px!important;width:{width}px!important;height:{height}px!important;box-sizing:border-box!important;margin:0!important;padding:0!important;border:0!important;background:transparent!important;color:transparent!important;text-shadow:none!important;overflow:hidden!important;white-space:nowrap!important;pointer-events:none!important;-webkit-appearance:none!important;appearance:none!important;"
        );
        let _ = element.set_attribute("style", &style);
    }

    fn semantic_value(runtime: &Runtime, id: WidgetId, semantics: &Semantics) -> Option<String> {
        if semantics.masked {
            return None;
        }
        if semantics.role == fission_ir::Role::TextInput {
            semantics.value.clone().or_else(|| {
                runtime
                    .runtime_state
                    .text_edit
                    .get(id)
                    .map(|state| state.committed_text())
            })
        } else {
            semantics.value.clone()
        }
    }

    fn collect_descendant_text(ir: &CoreIR, id: WidgetId) -> Option<String> {
        let mut text = String::new();
        collect_descendant_text_inner(ir, id, &mut text);
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    }

    fn collect_descendant_text_inner(ir: &CoreIR, id: WidgetId, output: &mut String) {
        let Some(node) = ir.nodes.get(&id) else {
            return;
        };
        match &node.op {
            Op::Paint(PaintOp::DrawText { text, .. }) if !text.is_empty() => {
                push_text(output, text)
            }
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                if !text.is_empty() {
                    push_text(output, &text);
                }
            }
            _ => {
                for child in &node.children {
                    collect_descendant_text_inner(ir, *child, output);
                }
            }
        }
    }

    fn push_text(output: &mut String, text: &str) {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(text);
    }

    fn focus_semantic_node(runtime: &mut Runtime, ir: &CoreIR, id: WidgetId) -> bool {
        let Some(node) = ir.nodes.get(&id) else {
            return false;
        };
        let Op::Semantics(semantics) = &node.op else {
            return false;
        };
        if !semantics.focusable || semantics.disabled {
            return false;
        }
        runtime
            .set_focused_widget(ir, Some(id), TextEditSource::Accessibility)
            .unwrap_or(false)
    }

    fn editable_text_input(ir: &CoreIR, focused: Option<WidgetId>) -> Option<(WidgetId, bool)> {
        let id = focused?;
        let node = ir.nodes.get(&id)?;
        let Op::Semantics(semantics) = &node.op else {
            return None;
        };
        (semantics.role == fission_ir::Role::TextInput
            && !semantics.read_only
            && !semantics.disabled)
            .then_some((id, semantics.multiline))
    }

    fn activate_semantic_node(runtime: &mut Runtime, ir: &CoreIR, id: WidgetId) -> bool {
        let Some(node) = ir.nodes.get(&id) else {
            return false;
        };
        let Op::Semantics(semantics) = &node.op else {
            return false;
        };
        if semantics.disabled {
            return false;
        }
        crate::dispatch_semantics_action(
            ir,
            runtime,
            id,
            semantics,
            fission_ir::semantics::ActionTrigger::Default,
            ActionInput::None,
        )
    }

    fn set_optional_attribute(element: &HtmlElement, name: &str, value: Option<&str>) {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            let _ = element.set_attribute(name, value);
        }
    }

    fn set_boolean_attribute(element: &HtmlElement, name: &str, enabled: bool) {
        if enabled {
            let _ = element.set_attribute(name, "true");
        }
    }

    fn bool_string(value: bool) -> &'static str {
        if value {
            "true"
        } else {
            "false"
        }
    }

    fn number_string(value: &f32) -> String {
        value.to_string()
    }

    fn restore_attribute(element: &HtmlCanvasElement, name: &str, value: Option<&str>) {
        if let Some(value) = value {
            let _ = element.set_attribute(name, value);
        } else {
            let _ = element.remove_attribute(name);
        }
    }

    fn restore_html_attribute(element: &HtmlElement, name: &str, value: Option<&str>) {
        if let Some(value) = value {
            let _ = element.set_attribute(name, value);
        } else {
            let _ = element.remove_attribute(name);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use imp::WebAccessibilityBridge;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_interactive_roles_and_states_to_browser_semantics() {
        let semantics = Semantics {
            role: Role::Checkbox,
            label: Some("Reinforce the mast".into()),
            checked: Some(true),
            focusable: true,
            required: true,
            ..Semantics::default()
        };
        let descriptor =
            SemanticDescriptor::new(&semantics, semantics.label.clone(), semantics.value.clone());
        assert_eq!(descriptor.role, Some("checkbox"));
        assert_eq!(
            descriptor.accessible_name.as_deref(),
            Some("Reinforce the mast")
        );
        assert_eq!(descriptor.checked, Some(true));
        assert!(descriptor.required);
    }

    #[test]
    fn exposes_static_text_without_promoting_it_to_a_control_role() {
        let semantics = Semantics {
            role: Role::Text,
            label: Some("The squall is rising".into()),
            ..Semantics::default()
        };
        let descriptor = SemanticDescriptor::new(&semantics, semantics.label.clone(), None);
        assert_eq!(descriptor.role, None);
        assert_eq!(
            descriptor.exposed_text.as_deref(),
            Some("The squall is rising")
        );
        assert_eq!(descriptor.accessible_name, None);
    }

    #[test]
    fn does_not_expose_masked_text_as_a_dom_value() {
        let semantics = Semantics {
            role: Role::TextInput,
            label: Some("Password".into()),
            value: Some("not-for-the-accessibility-tree".into()),
            masked: true,
            ..Semantics::default()
        };
        let descriptor = SemanticDescriptor::new(&semantics, semantics.label.clone(), None);
        assert_eq!(descriptor.role, Some("textbox"));
        assert_eq!(descriptor.value_text, None);
    }

    #[test]
    fn preserves_range_values_and_live_validation_state() {
        let slider = Semantics {
            role: Role::Slider,
            label: Some("Volume".into()),
            value: Some("forty percent".into()),
            min_value: Some(0.0),
            max_value: Some(100.0),
            current_value: Some(40.0),
            ..Semantics::default()
        };
        let descriptor =
            SemanticDescriptor::new(&slider, slider.label.clone(), slider.value.clone());
        assert_eq!(descriptor.role, Some("slider"));
        assert_eq!(descriptor.min_value, Some(0.0));
        assert_eq!(descriptor.max_value, Some(100.0));
        assert_eq!(descriptor.current_value, Some(40.0));
        assert_eq!(descriptor.value_text.as_deref(), Some("forty percent"));

        let invalid = Semantics {
            role: Role::TextInput,
            validation_state: TextFieldValidationState::Invalid,
            validation_message: Some("A call sign is required".into()),
            disabled: true,
            ..Semantics::default()
        };
        let descriptor = SemanticDescriptor::new(&invalid, None, None);
        assert!(descriptor.invalid);
        assert!(descriptor.live);
        assert!(descriptor.disabled);
        assert_eq!(
            descriptor.description.as_deref(),
            Some("A call sign is required")
        );
    }

    #[test]
    fn maps_browser_navigation_and_printable_keys() {
        assert_eq!(web_key("ArrowLeft"), Some((KeyCode::Left, None)));
        assert_eq!(web_key("Tab"), Some((KeyCode::Tab, None)));
        assert_eq!(web_key("é"), Some((KeyCode::Char('é'), Some("é".into()))));
        assert_eq!(web_key("Dead"), None);
    }

    #[test]
    fn leaves_tab_traversal_to_the_browser_except_inside_a_focus_barrier() {
        assert!(!bridge_owns_key(&KeyCode::Tab, false));
        assert!(bridge_owns_key(&KeyCode::Tab, true));
        assert!(bridge_owns_key(&KeyCode::Enter, false));
    }
}
