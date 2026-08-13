use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use fission_core::event::ImeEvent;
use fission_core::{ActionEnvelope, ActionId, ActionInput, InputEvent, Runtime};
use fission_ir::semantics::{ActionTrigger, Role};
use fission_ir::{CoreIR, Op, Semantics, WidgetId};
use fission_layout::{LayoutRect, LayoutSize, LayoutSnapshot};
use fission_test_driver::TestEvent;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    CompositionEvent, Document, Element, Event, EventTarget, FocusEvent, HtmlCanvasElement,
    HtmlElement, HtmlInputElement, HtmlTextAreaElement, KeyboardEvent,
};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::platform::web::WindowExtWebSys;
use winit::window::Window;

use super::model::{SemanticNode, SemanticSnapshot};
use crate::ime::TextInputConfig;
use crate::ime::{
    bind_web_text_control, byte_to_utf16, focus_web_text_control, refresh_web_ime_geometry,
    resume_web_ime, set_web_ime_composing, suspend_web_ime, unbind_web_text_control,
    update_web_ime_viewport, utf16_to_byte, web_ime_is_composing, WebTextControl,
};

type EventQueue = Rc<RefCell<VecDeque<WebAccessibilityEvent>>>;

#[derive(Debug)]
enum WebAccessibilityEvent {
    Activate(WidgetId),
    Focus(WidgetId),
    Blur(WidgetId),
    TextValue {
        target: WidgetId,
        value: String,
        anchor: usize,
        caret: usize,
    },
    Selection {
        target: WidgetId,
        anchor: usize,
        caret: usize,
    },
    Preedit {
        target: WidgetId,
        text: String,
        cursor: Option<(usize, usize)>,
    },
    Commit {
        target: WidgetId,
        text: String,
    },
    Cancel(WidgetId),
    AdjustValue {
        target: WidgetId,
        direction: f32,
    },
    SetValueBoundary {
        target: WidgetId,
        maximum: bool,
    },
    Scroll {
        target: WidgetId,
        horizontal: bool,
        command: ScrollCommand,
    },
    Submit(WidgetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollCommand {
    Backward,
    Forward,
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyboardIntent {
    Activate,
    Decrease,
    Increase,
    Minimum,
    Maximum,
    Scroll {
        horizontal: bool,
        command: ScrollCommand,
    },
    Submit,
}

pub struct AccessibilityBridge {
    proxy: EventLoopProxy<TestEvent>,
    events: EventQueue,
    dom: Option<WebAccessibilityDom>,
    suspended: bool,
    initialization_error_reported: bool,
}

impl AccessibilityBridge {
    pub fn new(proxy: EventLoopProxy<TestEvent>) -> Self {
        Self {
            proxy,
            events: Rc::new(RefCell::new(VecDeque::new())),
            dom: None,
            suspended: false,
            initialization_error_reported: false,
        }
    }

    pub fn ensure_adapter(&mut self, _event_loop: &ActiveEventLoop, window: &Window) {
        if self.dom.is_some() {
            return;
        }
        let Some(canvas) = window.canvas() else {
            self.report_initialization_error("the Winit window did not expose its canvas");
            return;
        };
        match WebAccessibilityDom::new(canvas, self.events.clone(), self.proxy.clone()) {
            Ok(dom) => {
                self.dom = Some(dom);
                self.initialization_error_reported = false;
            }
            Err(error) => self.report_initialization_error(&format_js_error(error)),
        }
    }

    pub fn process_window_event(&mut self, _window: &Window, event: &WindowEvent) {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            if let Some(dom) = self.dom.as_mut() {
                dom.refresh_root_geometry();
            }
            refresh_web_ime_geometry();
        }
    }

    pub fn update_tree(
        &mut self,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        runtime: &Runtime,
        scale_factor: f64,
    ) {
        let Some(dom) = self.dom.as_mut() else {
            return;
        };
        let focused_config = runtime
            .runtime_state
            .interaction
            .focused
            .and_then(|target| {
                semantics_for(ir, target)
                    .filter(|semantics| semantics.role == Role::TextInput)
                    .map(|semantics| runtime_text_config(runtime, target, semantics))
            });
        let snapshot = SemanticSnapshot::build(ir, layout, runtime);
        if let Err(error) = dom.update(&snapshot, scale_factor, self.suspended, focused_config) {
            web_sys::console::error_1(&JsValue::from_str(&format!(
                "fission-shell-winit: Web accessibility update failed: {}",
                format_js_error(error)
            )));
        }
    }

    pub fn drain_events(
        &mut self,
        runtime: &mut Runtime,
        ir: Option<&CoreIR>,
        layout: Option<&LayoutSnapshot>,
    ) -> bool {
        let (Some(ir), Some(layout)) = (ir, layout) else {
            self.events.borrow_mut().clear();
            return false;
        };
        let mut changed = false;
        loop {
            let event = self.events.borrow_mut().pop_front();
            let Some(event) = event else {
                break;
            };
            changed |= handle_event(event, runtime, ir, layout);
        }
        changed
    }

    pub(crate) fn focus_runtime_text_control(&mut self, runtime: &Runtime, ir: Option<&CoreIR>) {
        if let Some(dom) = self.dom.as_mut() {
            dom.focus_runtime_text_control(runtime, ir);
        }
    }

    pub fn resume(&mut self) {
        self.suspended = false;
        if let Some(dom) = self.dom.as_ref() {
            let _ = dom.root.style().set_property("display", "block");
        }
        resume_web_ime();
    }

    pub fn suspend(&mut self) {
        self.suspended = true;
        self.events.borrow_mut().clear();
        if let Some(dom) = self.dom.as_ref() {
            let _ = dom.root.style().set_property("display", "none");
        }
        suspend_web_ime();
        // Hiding or blurring the focused native control can synchronously emit
        // `focusout`. It belongs to suspension, not the next resumed frame.
        self.events.borrow_mut().clear();
    }

    pub fn shutdown(&mut self) {
        self.suspend();
        if let Some(mut dom) = self.dom.take() {
            dom.shutdown();
        }
        self.events.borrow_mut().clear();
    }

    fn report_initialization_error(&mut self, message: &str) {
        if self.initialization_error_reported {
            return;
        }
        self.initialization_error_reported = true;
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "fission-shell-winit: Web accessibility unavailable: {message}"
        )));
    }
}

pub fn window_must_start_hidden() -> bool {
    false
}

struct WebAccessibilityDom {
    document: Document,
    canvas: HtmlCanvasElement,
    root: HtmlElement,
    nodes: HashMap<WidgetId, RetainedNode>,
    listeners: Vec<DomListener>,
    bound_text_control: Option<WidgetId>,
    viewport: LayoutSize,
}

struct RetainedNode {
    element: HtmlElement,
    control: Option<WebTextControl>,
    control_multiline: bool,
}

impl WebAccessibilityDom {
    fn new(
        canvas: HtmlCanvasElement,
        events: EventQueue,
        proxy: EventLoopProxy<TestEvent>,
    ) -> Result<Self, JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("browser document is unavailable"))?;
        let root = document.create_element("div")?.dyn_into::<HtmlElement>()?;
        root.set_id("fission-accessibility-root");
        root.set_attribute("data-fission-accessibility", "true")?;
        apply_root_style(&root)?;

        let mut listeners = install_semantic_listeners(&root, events, proxy.clone())?;
        listeners.extend(install_geometry_listeners(
            root.clone(),
            canvas.clone(),
            proxy,
        )?);
        document
            .body()
            .ok_or_else(|| JsValue::from_str("document.body is unavailable"))?
            .append_child(&root)?;
        let mut dom = Self {
            document,
            canvas,
            root,
            nodes: HashMap::new(),
            listeners,
            bound_text_control: None,
            viewport: LayoutSize::default(),
        };
        dom.refresh_root_geometry();
        Ok(dom)
    }

    fn update(
        &mut self,
        snapshot: &SemanticSnapshot,
        scale_factor: f64,
        suspended: bool,
        focused_config: Option<TextInputConfig>,
    ) -> Result<(), JsValue> {
        self.viewport = snapshot.viewport;
        self.root.set_attribute(
            "data-fission-device-scale-factor",
            &scale_factor.to_string(),
        )?;
        self.root
            .style()
            .set_property("display", if suspended { "none" } else { "block" })?;
        self.refresh_root_geometry();

        let mut present = HashSet::with_capacity(snapshot.nodes.len());
        for node in &snapshot.nodes {
            present.insert(node.id);
            self.ensure_node(node)?;
        }
        // Existing elements retain their identity and event state, but their
        // DOM position must follow current semantic order after list reorders.
        for node in &snapshot.nodes {
            if let Some(retained) = self.nodes.get(&node.id) {
                self.root.append_child(&retained.element)?;
            }
        }
        self.remove_absent_nodes(&present);
        for node in &snapshot.nodes {
            self.update_node(node)?;
        }
        self.update_root_ownership(&snapshot.roots)?;

        let focused_text = snapshot
            .nodes
            .iter()
            .find(|node| {
                node.focused && node.is_text_control() && !node.disabled && !node.read_only
            })
            .map(|node| node.id);
        if self.bound_text_control != focused_text {
            if let Some(previous) = self.bound_text_control.take() {
                unbind_web_text_control(previous);
            }
        }
        if let Some(focused) = focused_text {
            if let Some(control) = self
                .nodes
                .get(&focused)
                .and_then(|node| node.control.clone())
            {
                bind_web_text_control(
                    focused,
                    control,
                    self.canvas.clone(),
                    snapshot.viewport,
                    focused_config,
                );
                self.bound_text_control = Some(focused);
                let _ = focus_web_text_control(focused);
            }
        }
        update_web_ime_viewport(self.canvas.clone(), snapshot.viewport);
        self.synchronize_dom_focus(snapshot.focused);
        Ok(())
    }

    pub(crate) fn focus_runtime_text_control(&mut self, runtime: &Runtime, ir: Option<&CoreIR>) {
        let Some(ir) = ir else {
            return;
        };
        let Some(target) = runtime.runtime_state.interaction.focused else {
            return;
        };
        let Some(semantics) = semantics_for(ir, target) else {
            return;
        };
        if semantics.role != Role::TextInput || semantics.disabled {
            return;
        }
        let Some(control) = self
            .nodes
            .get(&target)
            .and_then(|node| node.control.clone())
        else {
            return;
        };
        if semantics.read_only {
            let _ = control.html_element().focus();
        } else {
            bind_web_text_control(
                target,
                control,
                self.canvas.clone(),
                self.viewport,
                Some(runtime_text_config(runtime, target, semantics)),
            );
            self.bound_text_control = Some(target);
            let _ = focus_web_text_control(target);
        }
    }

    fn ensure_node(&mut self, semantic: &SemanticNode) -> Result<(), JsValue> {
        let document = self.document.clone();
        let is_bound = self.bound_text_control == Some(semantic.id);
        if let Some(existing) = self.nodes.get_mut(&semantic.id) {
            let wants_multiline = semantic.multiline;
            if semantic.is_text_control() {
                if existing.control.is_none() {
                    existing.control = Some(create_text_control(
                        &document,
                        semantic.id,
                        wants_multiline,
                    )?);
                    existing.control_multiline = wants_multiline;
                    if let Some(control) = existing.control.as_ref() {
                        existing.element.append_child(&control.html_element())?;
                    }
                } else if existing.control_multiline != wants_multiline && !is_bound {
                    if let Some(control) = existing.control.take() {
                        control.html_element().remove();
                    }
                    existing.control = Some(create_text_control(
                        &document,
                        semantic.id,
                        wants_multiline,
                    )?);
                    existing.control_multiline = wants_multiline;
                    if let Some(control) = existing.control.as_ref() {
                        existing.element.append_child(&control.html_element())?;
                    }
                }
            } else if !is_bound {
                if let Some(control) = existing.control.take() {
                    control.html_element().remove();
                }
            }
            return Ok(());
        }

        let element = self
            .document
            .create_element("div")?
            .dyn_into::<HtmlElement>()?;
        element.set_id(&dom_id(semantic.id));
        element.set_attribute("data-fission-widget", &widget_key(semantic.id))?;
        apply_node_style(&element)?;
        let control = if semantic.is_text_control() {
            let control = create_text_control(&self.document, semantic.id, semantic.multiline)?;
            element.append_child(&control.html_element())?;
            Some(control)
        } else {
            None
        };
        self.root.append_child(&element)?;
        self.nodes.insert(
            semantic.id,
            RetainedNode {
                element,
                control,
                control_multiline: semantic.multiline,
            },
        );
        Ok(())
    }

    fn update_node(&mut self, semantic: &SemanticNode) -> Result<(), JsValue> {
        let scale = self.scale();
        let ime_bound = self.bound_text_control == Some(semantic.id);
        let Some(node) = self.nodes.get(&semantic.id) else {
            return Ok(());
        };
        let element = &node.element;
        set_optional_attribute(element, "role", Some(dom_role(semantic)))?;
        set_optional_attribute(element, "aria-label", semantic_label(semantic).as_deref())?;
        set_optional_attribute(
            element,
            "data-fission-identifier",
            semantic.identifier.as_deref(),
        )?;
        set_bool_attribute(element, "aria-disabled", semantic.disabled)?;
        set_bool_attribute(element, "aria-readonly", semantic.read_only)?;
        set_optional_attribute(
            element,
            "aria-checked",
            semantic
                .checked
                .map(|checked| if checked { "true" } else { "false" }),
        )?;
        set_optional_attribute(
            element,
            "aria-valuemin",
            semantic.min_value.map(|value| value.to_string()).as_deref(),
        )?;
        set_optional_attribute(
            element,
            "aria-valuemax",
            semantic.max_value.map(|value| value.to_string()).as_deref(),
        )?;
        set_optional_attribute(
            element,
            "aria-valuenow",
            semantic
                .current_value
                .map(|value| value.to_string())
                .as_deref(),
        )?;
        let aria_value = (!semantic.is_text_control() && !semantic.masked)
            .then_some(semantic.value.as_deref())
            .flatten();
        set_optional_attribute(element, "aria-valuetext", aria_value)?;
        set_bool_attribute(element, "aria-multiline", semantic.multiline)?;
        set_optional_attribute(
            element,
            "tabindex",
            (!semantic.is_text_control() && semantic.focusable && !semantic.disabled)
                .then_some("0"),
        )?;
        set_bool_attribute(
            element,
            "data-fission-activatable",
            !semantic.disabled && semantic.supports(ActionTrigger::Default),
        )?;
        set_bool_attribute(element, "data-fission-scroll-x", semantic.scrollable_x)?;
        set_bool_attribute(element, "data-fission-scroll-y", semantic.scrollable_y)?;
        set_optional_attribute(
            element,
            "aria-owns",
            (!semantic.children.is_empty())
                .then(|| {
                    semantic
                        .children
                        .iter()
                        .map(|child| dom_id(*child))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .as_deref(),
        )?;
        if let Some((anchor, caret)) = semantic.selection {
            element.set_attribute("data-fission-selection-anchor", &anchor.to_string())?;
            element.set_attribute("data-fission-selection-caret", &caret.to_string())?;
        } else {
            element.remove_attribute("data-fission-selection-anchor")?;
            element.remove_attribute("data-fission-selection-caret")?;
        }
        position_node(element, semantic.visible_bounds, scale)?;
        if let Some(control) = node.control.as_ref() {
            update_text_control(control, semantic, ime_bound)?;
        }
        Ok(())
    }

    fn remove_absent_nodes(&mut self, present: &HashSet<WidgetId>) {
        let removed = self
            .nodes
            .keys()
            .filter(|id| !present.contains(id))
            .copied()
            .collect::<Vec<_>>();
        for id in removed {
            if self.bound_text_control == Some(id) {
                unbind_web_text_control(id);
                self.bound_text_control = None;
            }
            if let Some(node) = self.nodes.remove(&id) {
                node.element.remove();
            }
        }
    }

    fn update_root_ownership(&self, roots: &[WidgetId]) -> Result<(), JsValue> {
        let owns = roots
            .iter()
            .map(|id| dom_id(*id))
            .collect::<Vec<_>>()
            .join(" ");
        set_optional_attribute(
            &self.root,
            "aria-owns",
            (!owns.is_empty()).then_some(owns.as_str()),
        )
    }

    fn synchronize_dom_focus(&self, focused: Option<WidgetId>) {
        let active = self.document.active_element();
        let active_widget = active.as_ref().and_then(widget_from_element);
        // Do not steal focus from ordinary page chrome mounted around the
        // canvas. The bridge owns only elements in its retained mirror.
        if active_widget.is_none() && active.as_ref().is_some_and(is_page_control) {
            return;
        }
        if let Some(node) = focused.and_then(|id| self.nodes.get(&id)) {
            if node.element.get_attribute("aria-disabled").as_deref() == Some("true") {
                blur_owned_active_element(active.as_ref(), active_widget, &self.nodes);
            } else if let Some(control) = node.control.as_ref() {
                let control = control.html_element();
                if !active
                    .as_ref()
                    .is_some_and(|active| active.is_same_node(Some(&control)))
                {
                    let _ = control.focus();
                }
            } else if node.element.has_attribute("tabindex") {
                if !active
                    .as_ref()
                    .is_some_and(|active| active.is_same_node(Some(&node.element)))
                {
                    let _ = node.element.focus();
                }
            } else {
                blur_owned_active_element(active.as_ref(), active_widget, &self.nodes);
            }
        } else {
            blur_owned_active_element(active.as_ref(), active_widget, &self.nodes);
        }
    }

    fn refresh_root_geometry(&mut self) {
        synchronize_root_geometry(&self.root, &self.canvas);
        refresh_web_ime_geometry();
    }

    fn scale(&self) -> (f64, f64) {
        let rect = self.canvas.get_bounding_client_rect();
        (
            safe_scale(rect.width(), self.viewport.width),
            safe_scale(rect.height(), self.viewport.height),
        )
    }

    fn shutdown(&mut self) {
        if let Some(bound) = self.bound_text_control.take() {
            unbind_web_text_control(bound);
        }
        self.listeners.clear();
        self.nodes.clear();
        self.root.remove();
    }
}

fn runtime_text_config(
    runtime: &Runtime,
    target: WidgetId,
    semantics: &Semantics,
) -> TextInputConfig {
    let mut config = TextInputConfig::from_semantics(semantics);
    if let Some(state) = runtime.runtime_state.text_edit.get(target) {
        config.value = state.committed_text();
        config.selection = (state.anchor, state.caret);
        config.preedit_active = state.preedit.is_some();
    }
    config
}

fn is_page_control(element: &Element) -> bool {
    matches!(
        element.tag_name().as_str(),
        "A" | "BUTTON" | "INPUT" | "SELECT" | "TEXTAREA"
    ) || element.has_attribute("tabindex")
        || element.has_attribute("contenteditable")
}

fn install_semantic_listeners(
    root: &HtmlElement,
    events: EventQueue,
    proxy: EventLoopProxy<TestEvent>,
) -> Result<Vec<DomListener>, JsValue> {
    let target: EventTarget = root.clone().unchecked_into();
    let mut listeners = Vec::new();
    for event_name in [
        "click",
        "focusin",
        "focusout",
        "input",
        "select",
        "compositionstart",
        "compositionupdate",
        "compositionend",
        "keydown",
    ] {
        let event_queue = events.clone();
        let event_proxy = proxy.clone();
        listeners.push(DomListener::install(
            target.clone(),
            event_name,
            true,
            move |event| {
                handle_dom_event(event_name, event, &event_queue, &event_proxy);
            },
        )?);
    }
    Ok(listeners)
}

fn install_geometry_listeners(
    root: HtmlElement,
    canvas: HtmlCanvasElement,
    proxy: EventLoopProxy<TestEvent>,
) -> Result<Vec<DomListener>, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window is unavailable"))?;
    let target: EventTarget = window.unchecked_into();
    ["resize", "scroll"]
        .into_iter()
        .map(|event_name| {
            let event_proxy = proxy.clone();
            let root = root.clone();
            let canvas = canvas.clone();
            DomListener::install(target.clone(), event_name, true, move |_| {
                synchronize_root_geometry(&root, &canvas);
                refresh_web_ime_geometry();
                let _ = event_proxy.send_event(TestEvent::Wake);
            })
        })
        .collect()
}

fn handle_dom_event(
    event_name: &str,
    event: Event,
    events: &EventQueue,
    proxy: &EventLoopProxy<TestEvent>,
) {
    let Some(target) = event.target().and_then(event_target_element) else {
        return;
    };
    let Some(widget) = widget_from_element(&target) else {
        return;
    };
    let queued = match event_name {
        "click" => {
            focus_dom_text_control(&target);
            (!target.has_attribute("data-fission-ime-control"))
                .then_some(WebAccessibilityEvent::Activate(widget))
        }
        "focusin" => {
            focus_dom_text_control(&target);
            Some(WebAccessibilityEvent::Focus(widget))
        }
        "focusout" => {
            let related_widget = event
                .dyn_ref::<FocusEvent>()
                .and_then(FocusEvent::related_target)
                .and_then(event_target_element)
                .as_ref()
                .and_then(widget_from_element);
            (related_widget != Some(widget)).then_some(WebAccessibilityEvent::Blur(widget))
        }
        "input" => text_value_event(widget, &target, &event),
        "select" => selection_event(widget, &target),
        "compositionstart" => {
            if target.has_attribute("data-fission-ime-control") {
                set_web_ime_composing(true);
            }
            None
        }
        "compositionupdate" => {
            if !target.has_attribute("data-fission-ime-control") {
                return;
            }
            let text = event
                .dyn_ref::<CompositionEvent>()
                .map(CompositionEvent::data)
                .unwrap_or_default();
            let cursor = Some((text.len(), text.len()));
            Some(WebAccessibilityEvent::Preedit {
                target: widget,
                text,
                cursor,
            })
        }
        "compositionend" => {
            if !target.has_attribute("data-fission-ime-control") {
                return;
            }
            let text = event
                .dyn_ref::<CompositionEvent>()
                .map(CompositionEvent::data)
                .unwrap_or_default();
            set_web_ime_composing(false);
            if text.is_empty() {
                Some(WebAccessibilityEvent::Cancel(widget))
            } else {
                Some(WebAccessibilityEvent::Commit {
                    target: widget,
                    text,
                })
            }
        }
        "keydown" => keyboard_event(widget, &target, &event),
        _ => None,
    };
    if let Some(queued) = queued {
        events.borrow_mut().push_back(queued);
        let _ = proxy.send_event(TestEvent::Wake);
    }
}

fn text_value_event(
    widget: WidgetId,
    target: &Element,
    event: &Event,
) -> Option<WebAccessibilityEvent> {
    if event
        .dyn_ref::<web_sys::InputEvent>()
        .map(web_sys::InputEvent::is_composing)
        .unwrap_or(false)
    {
        return None;
    }
    let (value, selection) = control_value_and_selection(target)?;
    let (anchor_utf16, caret_utf16) = selection.unwrap_or_else(|| {
        let end = value.encode_utf16().count();
        (end, end)
    });
    Some(WebAccessibilityEvent::TextValue {
        target: widget,
        anchor: utf16_to_byte(&value, anchor_utf16),
        caret: utf16_to_byte(&value, caret_utf16),
        value,
    })
}

fn selection_event(widget: WidgetId, target: &Element) -> Option<WebAccessibilityEvent> {
    if web_ime_is_composing() {
        return None;
    }
    let (value, selection) = control_value_and_selection(target)?;
    let (anchor, caret) = selection?;
    Some(WebAccessibilityEvent::Selection {
        target: widget,
        anchor: utf16_to_byte(&value, anchor),
        caret: utf16_to_byte(&value, caret),
    })
}

fn keyboard_event(
    widget: WidgetId,
    target: &Element,
    event: &Event,
) -> Option<WebAccessibilityEvent> {
    let keyboard = event.dyn_ref::<KeyboardEvent>()?;
    let key = keyboard.key();
    let semantic_owner = target.closest("[data-fission-widget]").ok().flatten();
    if semantic_owner
        .as_ref()
        .and_then(|element| element.get_attribute("aria-disabled"))
        .as_deref()
        == Some("true")
    {
        return None;
    }
    let role = target
        .closest("[role]")
        .ok()
        .flatten()
        .and_then(|element| element.get_attribute("role"));
    let activatable = semantic_owner
        .as_ref()
        .is_some_and(|element| element.has_attribute("data-fission-activatable"));
    let scrollable_x = semantic_owner
        .as_ref()
        .is_some_and(|element| element.has_attribute("data-fission-scroll-x"));
    let scrollable_y = semantic_owner
        .as_ref()
        .is_some_and(|element| element.has_attribute("data-fission-scroll-y"));
    let is_text_control = target.has_attribute("data-fission-ime-control");
    let intent = keyboard_intent(
        role.as_deref(),
        activatable && !is_text_control,
        scrollable_x,
        scrollable_y,
        target.dyn_ref::<HtmlInputElement>().is_some(),
        key.as_str(),
        keyboard.is_composing(),
        keyboard.repeat(),
    );
    let queued = match intent {
        Some(KeyboardIntent::Activate) => Some(WebAccessibilityEvent::Activate(widget)),
        Some(KeyboardIntent::Decrease) => Some(WebAccessibilityEvent::AdjustValue {
            target: widget,
            direction: -1.0,
        }),
        Some(KeyboardIntent::Increase) => Some(WebAccessibilityEvent::AdjustValue {
            target: widget,
            direction: 1.0,
        }),
        Some(KeyboardIntent::Minimum) => Some(WebAccessibilityEvent::SetValueBoundary {
            target: widget,
            maximum: false,
        }),
        Some(KeyboardIntent::Maximum) => Some(WebAccessibilityEvent::SetValueBoundary {
            target: widget,
            maximum: true,
        }),
        Some(KeyboardIntent::Scroll {
            horizontal,
            command,
        }) => Some(WebAccessibilityEvent::Scroll {
            target: widget,
            horizontal,
            command,
        }),
        Some(KeyboardIntent::Submit) => Some(WebAccessibilityEvent::Submit(widget)),
        None => None,
    };
    if queued.is_some() {
        event.prevent_default();
        event.stop_propagation();
    }
    queued
}

fn keyboard_intent(
    role: Option<&str>,
    activatable: bool,
    scrollable_x: bool,
    scrollable_y: bool,
    single_line_text: bool,
    key: &str,
    composing: bool,
    repeat: bool,
) -> Option<KeyboardIntent> {
    if composing {
        return None;
    }
    if single_line_text && key == "Enter" {
        return (!repeat).then_some(KeyboardIntent::Submit);
    }
    let intent = match (role, key) {
        (Some("slider"), "ArrowDown" | "ArrowLeft" | "PageDown") => KeyboardIntent::Decrease,
        (Some("slider"), "ArrowUp" | "ArrowRight" | "PageUp") => KeyboardIntent::Increase,
        (Some("slider"), "Home") => KeyboardIntent::Minimum,
        (Some("slider"), "End") => KeyboardIntent::Maximum,
        (Some("button" | "menuitem"), "Enter" | " " | "Spacebar") => KeyboardIntent::Activate,
        (Some("link"), "Enter") => KeyboardIntent::Activate,
        (Some("checkbox" | "radio" | "switch"), " " | "Spacebar") => KeyboardIntent::Activate,
        (_, "Enter" | " " | "Spacebar") if activatable => KeyboardIntent::Activate,
        (_, "ArrowUp" | "PageUp") if scrollable_y => KeyboardIntent::Scroll {
            horizontal: false,
            command: ScrollCommand::Backward,
        },
        (_, "ArrowDown" | "PageDown") if scrollable_y => KeyboardIntent::Scroll {
            horizontal: false,
            command: ScrollCommand::Forward,
        },
        (_, "ArrowLeft") if scrollable_x => KeyboardIntent::Scroll {
            horizontal: true,
            command: ScrollCommand::Backward,
        },
        (_, "ArrowRight") if scrollable_x => KeyboardIntent::Scroll {
            horizontal: true,
            command: ScrollCommand::Forward,
        },
        (_, "Home") if scrollable_y || scrollable_x => KeyboardIntent::Scroll {
            horizontal: !scrollable_y,
            command: ScrollCommand::Start,
        },
        (_, "End") if scrollable_y || scrollable_x => KeyboardIntent::Scroll {
            horizontal: !scrollable_y,
            command: ScrollCommand::End,
        },
        _ => return None,
    };
    if repeat && intent == KeyboardIntent::Activate {
        None
    } else {
        Some(intent)
    }
}

fn control_value_and_selection(target: &Element) -> Option<(String, Option<(usize, usize)>)> {
    let direction = selection_direction(target);
    if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
        let selection = input
            .selection_start()
            .ok()
            .flatten()
            .zip(input.selection_end().ok().flatten())
            .map(|(start, end)| {
                directed_dom_selection(start as usize, end as usize, direction.as_deref())
            });
        return Some((input.value(), selection));
    }
    let textarea = target.dyn_ref::<HtmlTextAreaElement>()?;
    let selection = textarea
        .selection_start()
        .ok()
        .flatten()
        .zip(textarea.selection_end().ok().flatten())
        .map(|(start, end)| {
            directed_dom_selection(start as usize, end as usize, direction.as_deref())
        });
    Some((textarea.value(), selection))
}

fn selection_direction(target: &Element) -> Option<String> {
    js_sys::Reflect::get(target.as_ref(), &JsValue::from_str("selectionDirection"))
        .ok()
        .and_then(|value| value.as_string())
}

fn directed_dom_selection(start: usize, end: usize, direction: Option<&str>) -> (usize, usize) {
    if direction == Some("backward") {
        (end, start)
    } else {
        (start, end)
    }
}

fn handle_event(
    event: WebAccessibilityEvent,
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
) -> bool {
    match event {
        WebAccessibilityEvent::Activate(target) => semantics_for(ir, target)
            .map(|semantics| {
                dispatch_semantics_action(
                    runtime,
                    target,
                    semantics,
                    ActionTrigger::Default,
                    ActionInput::None,
                )
            })
            .unwrap_or(false),
        WebAccessibilityEvent::Focus(target) => set_focus(runtime, ir, Some(target)),
        WebAccessibilityEvent::Blur(target) => {
            (runtime.runtime_state.interaction.focused == Some(target))
                && set_focus(runtime, ir, None)
        }
        WebAccessibilityEvent::TextValue {
            target,
            value,
            anchor,
            caret,
        } => set_text_value(runtime, ir, layout, target, &value, anchor, caret),
        WebAccessibilityEvent::Selection {
            target,
            anchor,
            caret,
        } => set_text_selection(runtime, ir, target, anchor, caret),
        WebAccessibilityEvent::Preedit {
            target,
            text,
            cursor,
        } => handle_ime(
            runtime,
            ir,
            layout,
            target,
            ImeEvent::Preedit { text, cursor },
        ),
        WebAccessibilityEvent::Commit { target, text } => {
            handle_ime(runtime, ir, layout, target, ImeEvent::Commit { text })
        }
        WebAccessibilityEvent::Cancel(target) => {
            handle_ime(runtime, ir, layout, target, ImeEvent::Cancel)
        }
        WebAccessibilityEvent::AdjustValue { target, direction } => semantics_for(ir, target)
            .map(|semantics| adjust_numeric_value(runtime, target, semantics, direction))
            .unwrap_or(false),
        WebAccessibilityEvent::SetValueBoundary { target, maximum } => semantics_for(ir, target)
            .map(|semantics| set_numeric_boundary(runtime, target, semantics, maximum))
            .unwrap_or(false),
        WebAccessibilityEvent::Scroll {
            target,
            horizontal,
            command,
        } => scroll_semantic(runtime, ir, layout, target, horizontal, command),
        WebAccessibilityEvent::Submit(target) => semantics_for(ir, target)
            .map(|semantics| {
                let submitted = dispatch_semantics_action(
                    runtime,
                    target,
                    semantics,
                    ActionTrigger::Submit,
                    ActionInput::None,
                );
                dispatch_semantics_action(
                    runtime,
                    target,
                    semantics,
                    ActionTrigger::EditingComplete,
                    ActionInput::None,
                ) || submitted
            })
            .unwrap_or(false),
    }
}

fn set_text_value(
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    target: WidgetId,
    value: &str,
    anchor: usize,
    caret: usize,
) -> bool {
    let Some(semantics) = semantics_for(ir, target) else {
        return false;
    };
    if semantics.role != Role::TextInput || semantics.disabled || semantics.read_only {
        return false;
    }
    let mut changed = set_focus(runtime, ir, Some(target));
    runtime.runtime_state.text_edit.sync_from_runtime(
        target,
        semantics.value.as_deref().unwrap_or_default(),
        None,
        None,
    );
    let current = runtime
        .runtime_state
        .text_edit
        .get(target)
        .map(|state| state.committed_text())
        .unwrap_or_default();
    if current != value {
        let (old_start, old_end, inserted) = replacement_delta(&current, value);
        let state = runtime.runtime_state.text_edit.get_mut_or_default(target);
        state.caret = old_end;
        state.anchor = old_start;
        state.clear_preedit();
        changed |= runtime
            .handle_input(
                InputEvent::Ime(ImeEvent::Commit { text: inserted }),
                ir,
                layout,
            )
            .is_ok();
    }
    changed | set_text_selection(runtime, ir, target, anchor, caret)
}

fn handle_ime(
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    target: WidgetId,
    event: ImeEvent,
) -> bool {
    let Some(semantics) = semantics_for(ir, target) else {
        return false;
    };
    if semantics.role != Role::TextInput || semantics.disabled || semantics.read_only {
        return false;
    }
    let focused = set_focus(runtime, ir, Some(target));
    runtime
        .handle_input(InputEvent::Ime(event), ir, layout)
        .map(|_| true)
        .unwrap_or(focused)
}

fn set_text_selection(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    anchor: usize,
    caret: usize,
) -> bool {
    let Some(semantics) = semantics_for(ir, target) else {
        return false;
    };
    if semantics.role != Role::TextInput || semantics.disabled || semantics.read_only {
        return false;
    }
    let mut changed = set_focus(runtime, ir, Some(target));
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
    let anchor = clamp_boundary(&value, anchor);
    let caret = clamp_boundary(&value, caret);
    let old_selection = runtime
        .runtime_state
        .text_edit
        .get(target)
        .map(|state| (state.anchor, state.caret));
    if old_selection == Some((anchor, caret)) {
        return changed;
    }
    let state = runtime.runtime_state.text_edit.get_mut_or_default(target);
    state.caret = caret;
    state.anchor = anchor;
    state.clear_preedit();
    changed |= dispatch_cursor_change(runtime, target, semantics, caret, anchor);
    changed
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
            );
        }
    }
    runtime.runtime_state.interaction.set_focused(focus);
    if let Some(handler) = &runtime.ime_handler {
        let allowed = focus
            .and_then(|id| semantics_for(ir, id))
            .map(|semantics| {
                semantics.role == Role::TextInput && !semantics.disabled && !semantics.read_only
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
            );
        }
    }
    true
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
    if runtime
        .runtime_state
        .text_edit
        .get(target)
        .is_some_and(|state| state.last_dispatched_cursor == Some((caret, anchor)))
    {
        return false;
    }
    let Ok(payload) = serde_json::to_vec(&fission_core::action::CursorChanged { caret, anchor })
    else {
        return false;
    };
    runtime
        .runtime_state
        .text_edit
        .get_mut_or_default(target)
        .last_dispatched_cursor = Some((caret, anchor));
    dispatch_entry(
        runtime,
        target,
        semantics,
        entry.action_id,
        payload,
        ActionInput::None,
    )
}

fn adjust_numeric_value(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    direction: f32,
) -> bool {
    let Some(current) = semantics.current_value.filter(|value| value.is_finite()) else {
        return false;
    };
    let min = semantics
        .min_value
        .filter(|value| value.is_finite())
        .unwrap_or(f32::NEG_INFINITY);
    let max = semantics
        .max_value
        .filter(|value| value.is_finite())
        .unwrap_or(f32::INFINITY);
    if min > max {
        return false;
    }
    let value = (current + direction).clamp(min, max);
    if !value.is_finite() || value == current {
        return false;
    }
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Change)
    else {
        return false;
    };
    let Ok(payload) = serde_json::to_vec(&value) else {
        return false;
    };
    dispatch_entry(
        runtime,
        target,
        semantics,
        entry.action_id,
        payload,
        ActionInput::None,
    )
}

fn set_numeric_boundary(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    maximum: bool,
) -> bool {
    let value = if maximum {
        semantics.max_value
    } else {
        semantics.min_value
    };
    let Some(value) = value.filter(|value| value.is_finite()) else {
        return false;
    };
    if semantics
        .current_value
        .filter(|current| current.is_finite())
        == Some(value)
    {
        return false;
    }
    if maximum
        && semantics
            .min_value
            .is_some_and(|minimum| !minimum.is_finite() || value < minimum)
    {
        return false;
    }
    if !maximum
        && semantics
            .max_value
            .is_some_and(|maximum| !maximum.is_finite() || value > maximum)
    {
        return false;
    }
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Change)
    else {
        return false;
    };
    let Ok(payload) = serde_json::to_vec(&value) else {
        return false;
    };
    dispatch_entry(
        runtime,
        target,
        semantics,
        entry.action_id,
        payload,
        ActionInput::None,
    )
}

fn scroll_semantic(
    runtime: &mut Runtime,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    target: WidgetId,
    horizontal: bool,
    command: ScrollCommand,
) -> bool {
    let Some(semantics) = semantics_for(ir, target) else {
        return false;
    };
    if semantics.disabled
        || (horizontal && !semantics.scrollable_x)
        || (!horizontal && !semantics.scrollable_y)
    {
        return false;
    }
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
    let next = match command {
        ScrollCommand::Backward => current - viewport * 0.8,
        ScrollCommand::Forward => current + viewport * 0.8,
        ScrollCommand::Start => 0.0,
        ScrollCommand::End => max,
    }
    .clamp(0.0, max);
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
    dispatch_entry(
        runtime,
        target,
        semantics,
        entry.action_id,
        entry.payload_data.clone().unwrap_or_default(),
        input,
    )
}

fn dispatch_entry(
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    action_id: u128,
    payload: Vec<u8>,
    input: ActionInput,
) -> bool {
    let input = if let Some(scope_id) = semantics.action_scope_id {
        ActionInput::scoped_raw(scope_id, target, input)
    } else {
        input
    };
    runtime
        .dispatch_with_input(
            ActionEnvelope {
                id: ActionId::from_u128(action_id),
                payload,
            },
            target,
            &input,
        )
        .is_ok()
}

fn semantics_for(ir: &CoreIR, id: WidgetId) -> Option<&Semantics> {
    ir.nodes.get(&id).and_then(|node| match &node.op {
        Op::Semantics(semantics) => Some(semantics),
        _ => None,
    })
}

fn replacement_delta(old: &str, new: &str) -> (usize, usize, String) {
    let prefix = old
        .char_indices()
        .zip(new.char_indices())
        .take_while(|((_, old), (_, new))| old == new)
        .map(|((old_index, old), _)| old_index + old.len_utf8())
        .last()
        .unwrap_or(0);
    let old_tail = &old[prefix..];
    let new_tail = &new[prefix..];
    let suffix_chars = old_tail
        .chars()
        .rev()
        .zip(new_tail.chars().rev())
        .take_while(|(old, new)| old == new)
        .count();
    let old_suffix = if suffix_chars == 0 {
        0
    } else {
        old_tail
            .char_indices()
            .rev()
            .nth(suffix_chars - 1)
            .map(|(index, _)| old_tail.len() - index)
            .unwrap_or(0)
    };
    let new_suffix = if suffix_chars == 0 {
        0
    } else {
        new_tail
            .char_indices()
            .rev()
            .nth(suffix_chars - 1)
            .map(|(index, _)| new_tail.len() - index)
            .unwrap_or(0)
    };
    let old_end = old.len().saturating_sub(old_suffix);
    let new_end = new.len().saturating_sub(new_suffix);
    (prefix, old_end, new[prefix..new_end].to_string())
}

fn clamp_boundary(value: &str, offset: usize) -> usize {
    let mut offset = offset.min(value.len());
    while offset > 0 && !value.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

struct DomListener {
    target: EventTarget,
    event: &'static str,
    capture: bool,
    callback: Closure<dyn FnMut(Event)>,
}

impl DomListener {
    fn install(
        target: EventTarget,
        event: &'static str,
        capture: bool,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<Self, JsValue> {
        let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback_and_bool(
            event,
            callback.as_ref().unchecked_ref(),
            capture,
        )?;
        Ok(Self {
            target,
            event,
            capture,
            callback,
        })
    }
}

impl Drop for DomListener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            self.event,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        );
    }
}

fn create_text_control(
    document: &Document,
    widget: WidgetId,
    multiline: bool,
) -> Result<WebTextControl, JsValue> {
    let control = if multiline {
        let textarea = document
            .create_element("textarea")?
            .dyn_into::<HtmlTextAreaElement>()?;
        textarea.set_disabled(true);
        WebTextControl::Textarea(textarea)
    } else {
        let input = document
            .create_element("input")?
            .dyn_into::<HtmlInputElement>()?;
        input.set_disabled(true);
        WebTextControl::Input(input)
    };
    let element = control.html_element();
    element.set_attribute("data-fission-widget", &widget_key(widget))?;
    element.set_attribute("data-fission-ime-control", "true")?;
    element.set_attribute("tabindex", "-1")?;
    for (name, value) in [
        ("position", "absolute"),
        ("inset", "0"),
        ("width", "100%"),
        ("height", "100%"),
        ("opacity", "0"),
        ("pointer-events", "none"),
        ("border", "0"),
        ("padding", "0"),
        ("margin", "0"),
    ] {
        element.style().set_property(name, value)?;
    }
    Ok(control)
}

fn update_text_control(
    control: &WebTextControl,
    semantic: &SemanticNode,
    ime_bound: bool,
) -> Result<(), JsValue> {
    let element = control.html_element();
    set_optional_attribute(&element, "role", semantic.masked.then_some("textbox"))?;
    set_optional_attribute(&element, "aria-label", semantic_label(semantic).as_deref())?;
    set_optional_attribute(
        &element,
        "data-fission-identifier",
        semantic.identifier.as_deref(),
    )?;
    set_optional_attribute(
        &element,
        "tabindex",
        (semantic.focusable && !semantic.disabled).then_some("0"),
    )?;
    set_optional_attribute(
        &element,
        "aria-multiline",
        semantic.multiline.then_some("true"),
    )?;
    control.set_disabled(semantic.disabled);
    control.set_read_only(semantic.read_only);
    if let WebTextControl::Input(input) = control {
        input.set_type(if semantic.masked {
            "password"
        } else {
            match semantic.text_input_type {
                fission_ir::semantics::TextInputType::EmailAddress => "email",
                fission_ir::semantics::TextInputType::Url => "url",
                fission_ir::semantics::TextInputType::Phone => "tel",
                _ => "text",
            }
        });
    }
    if !ime_bound {
        let value = semantic.value.as_deref().unwrap_or_default();
        if control.value() != value {
            control.set_value(value);
        }
        if let Some((anchor, caret)) = semantic.selection {
            control.set_selection_utf16(byte_to_utf16(value, anchor), byte_to_utf16(value, caret));
        }
        let style = element.style();
        style.set_property("position", "absolute")?;
        style.set_property("inset", "0")?;
        style.set_property("width", "100%")?;
        style.set_property("height", "100%")?;
    }
    Ok(())
}

fn focus_dom_text_control(target: &Element) {
    let Some(owner) = target.closest("[data-fission-widget]").ok().flatten() else {
        return;
    };
    let semantic_owner = if owner.has_attribute("data-fission-ime-control") {
        owner.parent_element().unwrap_or_else(|| owner.clone())
    } else {
        owner.clone()
    };
    if semantic_owner.get_attribute("aria-disabled").as_deref() == Some("true") {
        return;
    }
    let read_only = semantic_owner.get_attribute("aria-readonly").as_deref() == Some("true");
    let control = if owner.has_attribute("data-fission-ime-control") {
        Some(owner)
    } else {
        owner
            .query_selector("[data-fission-ime-control]")
            .ok()
            .flatten()
    };
    let Some(control) = control else {
        return;
    };
    if let Some(input) = control.dyn_ref::<HtmlInputElement>() {
        input.set_disabled(false);
        input.set_read_only(read_only);
    } else if let Some(textarea) = control.dyn_ref::<HtmlTextAreaElement>() {
        textarea.set_disabled(false);
        textarea.set_read_only(read_only);
    }
    if let Some(element) = control.dyn_ref::<HtmlElement>() {
        let _ = element.focus();
    }
}

fn blur_owned_active_element(
    active: Option<&Element>,
    active_widget: Option<WidgetId>,
    nodes: &HashMap<WidgetId, RetainedNode>,
) {
    if active_widget.is_some_and(|id| nodes.contains_key(&id)) {
        if let Some(element) = active.and_then(|element| element.dyn_ref::<HtmlElement>()) {
            let _ = element.blur();
        }
    }
}

fn synchronize_root_geometry(root: &HtmlElement, canvas: &HtmlCanvasElement) {
    let rect = canvas.get_bounding_client_rect();
    let style = root.style();
    let _ = style.set_property("left", &format!("{}px", rect.left()));
    let _ = style.set_property("top", &format!("{}px", rect.top()));
    let _ = style.set_property("width", &format!("{}px", rect.width().max(0.0)));
    let _ = style.set_property("height", &format!("{}px", rect.height().max(0.0)));
}

fn apply_root_style(root: &HtmlElement) -> Result<(), JsValue> {
    for (name, value) in [
        ("position", "fixed"),
        ("overflow", "hidden"),
        ("pointer-events", "none"),
        ("z-index", "2147483646"),
        ("background", "transparent"),
        ("margin", "0"),
        ("padding", "0"),
        ("border", "0"),
    ] {
        root.style().set_property(name, value)?;
    }
    Ok(())
}

fn apply_node_style(node: &HtmlElement) -> Result<(), JsValue> {
    for (name, value) in [
        ("position", "absolute"),
        ("pointer-events", "none"),
        ("background", "transparent"),
        ("margin", "0"),
        ("padding", "0"),
        ("border", "0"),
        ("outline", "0"),
    ] {
        node.style().set_property(name, value)?;
    }
    Ok(())
}

fn position_node(
    element: &HtmlElement,
    bounds: Option<LayoutRect>,
    scale: (f64, f64),
) -> Result<(), JsValue> {
    let Some(bounds) = bounds else {
        element.style().set_property("display", "none")?;
        return Ok(());
    };
    let style = element.style();
    style.set_property("display", "block")?;
    style.set_property("left", &format!("{}px", f64::from(bounds.x()) * scale.0))?;
    style.set_property("top", &format!("{}px", f64::from(bounds.y()) * scale.1))?;
    style.set_property(
        "width",
        &format!("{}px", (f64::from(bounds.width()) * scale.0).max(0.0)),
    )?;
    style.set_property(
        "height",
        &format!("{}px", (f64::from(bounds.height()) * scale.1).max(0.0)),
    )?;
    Ok(())
}

fn dom_role(node: &SemanticNode) -> &'static str {
    match node.role {
        Role::Button => "button",
        Role::Link => "link",
        Role::MenuItem => "menuitem",
        Role::Text => "text",
        Role::TextInput => "presentation",
        Role::Image => "img",
        Role::Checkbox => "checkbox",
        Role::Radio => "radio",
        Role::Switch => "switch",
        Role::Dialog => "dialog",
        Role::Slider => "slider",
        Role::Input => "textbox",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::Generic => "group",
    }
}

fn semantic_label(node: &SemanticNode) -> Option<String> {
    match node.role {
        Role::Text => node.value.clone().or_else(|| node.label.clone()),
        _ => node.label.clone(),
    }
}

fn set_bool_attribute(element: &Element, name: &str, value: bool) -> Result<(), JsValue> {
    set_optional_attribute(element, name, value.then_some("true"))
}

fn set_optional_attribute(
    element: &Element,
    name: &str,
    value: Option<&str>,
) -> Result<(), JsValue> {
    if let Some(value) = value {
        element.set_attribute(name, value)
    } else {
        element.remove_attribute(name)
    }
}

fn dom_id(widget: WidgetId) -> String {
    format!("fission-a11y-{}", widget_key(widget))
}

fn widget_key(widget: WidgetId) -> String {
    format!("{:032x}", widget.as_u128())
}

fn widget_from_element(element: &Element) -> Option<WidgetId> {
    let owner = element.closest("[data-fission-widget]").ok().flatten()?;
    let raw = owner.get_attribute("data-fission-widget")?;
    u128::from_str_radix(&raw, 16).ok().map(WidgetId::from_u128)
}

fn event_target_element(target: EventTarget) -> Option<Element> {
    target.dyn_into::<Element>().ok()
}

fn safe_scale(css_extent: f64, logical_extent: f32) -> f64 {
    if css_extent.is_finite()
        && css_extent > 0.0
        && logical_extent.is_finite()
        && logical_extent > 0.0
    {
        css_extent / f64::from(logical_extent)
    } else {
        1.0
    }
}

fn format_js_error(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
