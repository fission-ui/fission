#![cfg(target_arch = "wasm32")]

//! Browser DOM projection for Fission's retained semantic tree.
//!
//! The canvas remains the visual authority. This projection provides
//! browser-native names, roles, relationships, keyboard focus, native control
//! behavior, and link behavior from the same semantic records.

use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    rc::Rc,
};

use fission_core::LinkTarget;
use fission_ir::{
    ActionTrigger, CoreIR, Op, PaintOp, Role, Semantics, TextFieldValidationState, TextInputType,
    WidgetId,
};
use fission_render::LayoutSize;
use fission_test_driver::TestEvent;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{
    Element, Event, HtmlCanvasElement, HtmlElement, HtmlInputElement, HtmlTextAreaElement,
    InputEvent, KeyboardEvent, MouseEvent,
};
use winit::event_loop::EventLoopProxy;

use crate::semantic_records::SemanticRecord;

const ROOT_STYLE: &str =
    "position:fixed;inset:0;overflow:hidden;pointer-events:none;z-index:2147483646;";

#[derive(Clone, Debug, PartialEq, Eq)]
struct NodeDescription {
    widget_id: WidgetId,
    parent: Option<WidgetId>,
    tag: &'static str,
    attributes: BTreeMap<String, String>,
    text_content: Option<String>,
    native_value: Option<String>,
    intercept_click: Option<bool>,
    pointer_interactive: bool,
    captures_native_input: bool,
}

struct ActivationListener {
    intercept: Rc<Cell<bool>>,
    callback: Closure<dyn FnMut(MouseEvent)>,
}

struct FocusListeners {
    focus_in: Closure<dyn FnMut(Event)>,
    focus_out: Closure<dyn FnMut(Event)>,
}

struct NativeInputListeners {
    input: Closure<dyn FnMut(InputEvent)>,
}

pub(crate) enum WebAccessibilityEvent {
    Activate(WidgetId),
    Focus(WidgetId),
    Blur(WidgetId),
    NativeInput(NativeInputValue),
    TraverseFocus { reverse: bool },
}

pub(crate) struct NativeInputValue {
    pub(crate) widget_id: WidgetId,
    pub(crate) value: String,
    pub(crate) selection_start: Option<u32>,
    pub(crate) selection_end: Option<u32>,
    pub(crate) selection_direction: Option<String>,
    pub(crate) input_type: String,
    pub(crate) is_composing: bool,
}

struct MountedNode {
    widget_id: WidgetId,
    element: Element,
    tag: &'static str,
    parent: Option<WidgetId>,
    attributes: BTreeMap<String, String>,
    text_content: Option<String>,
    native_value: Option<String>,
    activation: Option<ActivationListener>,
    focus: Option<FocusListeners>,
    native_input: Option<NativeInputListeners>,
}

/// Keyed DOM projection for one Fission canvas.
///
/// Nodes are reused by `WidgetId` and attributes are only changed when their
/// semantic values change. This keeps browser focus stable across ordinary
/// retained-tree refreshes. Links are projected here, so a semantic link has
/// one browser anchor rather than a separate accessibility and link overlay.
pub(crate) struct WebAccessibilityProjection {
    canvas: HtmlCanvasElement,
    root: HtmlElement,
    nodes: HashMap<WidgetId, MountedNode>,
    events: Rc<std::cell::RefCell<VecDeque<WebAccessibilityEvent>>>,
    suppress_focus_events: Rc<Cell<bool>>,
    proxy: EventLoopProxy<TestEvent>,
    original_canvas_aria_hidden: Option<String>,
    original_canvas_role: Option<String>,
    keydown: Closure<dyn FnMut(KeyboardEvent)>,
}

impl WebAccessibilityProjection {
    pub(crate) fn new(
        canvas: HtmlCanvasElement,
        proxy: EventLoopProxy<TestEvent>,
    ) -> Result<Self, String> {
        let document = canvas
            .owner_document()
            .ok_or_else(|| "web canvas has no owner document".to_string())?;
        let root = document
            .create_element("div")
            .map_err(crate::js_error_to_string)?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "browser created a non-HTML accessibility root".to_string())?;
        root.set_attribute("data-fission-accessibility-overlay", "")
            .map_err(crate::js_error_to_string)?;
        root.set_attribute("role", "none")
            .map_err(crate::js_error_to_string)?;
        root.set_attribute("style", ROOT_STYLE)
            .map_err(crate::js_error_to_string)?;
        let events = Rc::new(std::cell::RefCell::new(VecDeque::new()));
        let keydown_events = events.clone();
        let keydown_proxy = proxy.clone();
        let keydown = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            if event.key() != "Tab" || event.default_prevented() {
                return;
            }
            event.prevent_default();
            keydown_events
                .borrow_mut()
                .push_back(WebAccessibilityEvent::TraverseFocus {
                    reverse: event.shift_key(),
                });
            crate::send_wake_event(&keydown_proxy);
        }) as Box<dyn FnMut(_)>);
        root.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
        document
            .body()
            .ok_or_else(|| "browser document has no body".to_string())?
            .append_child(&root)
            .map_err(crate::js_error_to_string)?;

        let original_canvas_aria_hidden = canvas.get_attribute("aria-hidden");
        let original_canvas_role = canvas.get_attribute("role");
        canvas
            .set_attribute("aria-hidden", "true")
            .map_err(crate::js_error_to_string)?;
        canvas
            .set_attribute("role", "presentation")
            .map_err(crate::js_error_to_string)?;

        Ok(Self {
            canvas,
            root,
            nodes: HashMap::new(),
            events,
            suppress_focus_events: Rc::new(Cell::new(false)),
            proxy,
            original_canvas_aria_hidden,
            original_canvas_role,
            keydown,
        })
    }

    /// Synchronizes active visible semantic records after layout. Record input
    /// order is ignored; ordering and parentage come from `CoreIR.children`.
    pub(crate) fn sync(
        &mut self,
        ir: &CoreIR,
        records: &[SemanticRecord],
        viewport: LayoutSize,
        focused_widget: Option<WidgetId>,
    ) -> Result<(), String> {
        let viewport_valid = viewport.width > 0.0 && viewport.height > 0.0;
        let active = records
            .iter()
            .filter(|_| viewport_valid)
            .filter(|record| record.node.visible_bounds.is_some())
            .filter(|record| should_expose_semantics(&record.semantics))
            .map(|record| (record.id, record))
            .collect::<HashMap<_, _>>();

        let order = ordered_widget_ids(ir, &active.keys().copied().collect());
        let active_ids = order.iter().copied().collect::<HashSet<_>>();
        let mut child_order = HashMap::<Option<WidgetId>, Vec<WidgetId>>::new();
        let mut descriptions = Vec::with_capacity(order.len());

        let canvas_rect = self.canvas.get_bounding_client_rect();
        let scale_x = canvas_rect.width() / f64::from(viewport.width);
        let scale_y = canvas_rect.height() / f64::from(viewport.height);

        for widget_id in order {
            let record = active
                .get(&widget_id)
                .expect("ordered semantic IDs originate in the active record set");
            let parent = nearest_projected_parent(ir, widget_id, &active_ids);
            child_order.entry(parent).or_default().push(widget_id);
            let fallback_name = fallback_accessible_name(ir, widget_id, &record.semantics);
            let mut description =
                describe_semantics(widget_id, &record.semantics, fallback_name, &active_ids);
            description.parent = parent;
            let bounds = record
                .node
                .visible_bounds
                .expect("hidden semantics were removed from the active record set");
            description.attributes.insert(
                "style".to_owned(),
                geometry_style(
                    f64::from(bounds.x),
                    f64::from(bounds.y),
                    f64::from(bounds.width),
                    f64::from(bounds.height),
                    canvas_rect.left(),
                    canvas_rect.top(),
                    scale_x,
                    scale_y,
                    description.pointer_interactive && !record.semantics.disabled,
                ),
            );
            descriptions.push(description);
        }

        let document = self
            .root
            .owner_document()
            .ok_or_else(|| "accessibility overlay has no owner document".to_string())?;
        let active_element = document.active_element();
        let previous_projected_focus = active_element.as_ref().and_then(|element| {
            self.nodes.iter().find_map(|(widget_id, mounted)| {
                mounted
                    .element
                    .is_same_node(Some(element.as_ref()))
                    .then_some(*widget_id)
            })
        });
        let mut remaining = std::mem::take(&mut self.nodes);
        let mut next = HashMap::with_capacity(descriptions.len());
        for description in descriptions {
            let mounted = match remaining.remove(&description.widget_id) {
                Some(mut mounted) if mounted.tag == description.tag => {
                    self.update_mounted_node(&mut mounted, &description)?;
                    mounted
                }
                Some(mounted) => {
                    remove_mounted_node(mounted)?;
                    let element = create_element(&document, &description)?;
                    mount_node(
                        element,
                        &description,
                        &self.events,
                        &self.proxy,
                        &self.suppress_focus_events,
                    )?
                }
                None => {
                    let element = create_element(&document, &description)?;
                    mount_node(
                        element,
                        &description,
                        &self.events,
                        &self.proxy,
                        &self.suppress_focus_events,
                    )?
                }
            };
            next.insert(description.widget_id, mounted);
        }
        for (_, mounted) in remaining {
            remove_mounted_node(mounted)?;
        }
        self.nodes = next;
        if let Some(widget_id) = previous_projected_focus
            && !active_ids.contains(&widget_id)
        {
            self.events
                .borrow_mut()
                .push_back(WebAccessibilityEvent::Blur(widget_id));
            crate::send_wake_event(&self.proxy);
        }

        self.reconcile_children(None, &child_order)?;
        for parent in child_order.keys().flatten().copied().collect::<Vec<_>>() {
            self.reconcile_children(Some(parent), &child_order)?;
        }

        let current_projected_focus = active_element.as_ref().and_then(|element| {
            self.nodes.iter().find_map(|(widget_id, mounted)| {
                mounted
                    .element
                    .is_same_node(Some(element.as_ref()))
                    .then_some(*widget_id)
            })
        });
        let target_focus = focused_widget.filter(|widget_id| self.nodes.contains_key(widget_id));
        if current_projected_focus != target_focus {
            self.suppress_focus_events.set(true);
            if let Some(focused_widget) = target_focus {
                if let Some(mounted) = self.nodes.get(&focused_widget)
                    && let Ok(html) = mounted.element.clone().dyn_into::<HtmlElement>()
                {
                    let _ = html.focus();
                }
            } else if let Some(element) = active_element
                && self.root.contains(Some(element.as_ref()))
                && let Ok(html) = element.dyn_into::<HtmlElement>()
            {
                html.blur();
            }
            self.suppress_focus_events.set(false);
        }

        Ok(())
    }

    /// Drains browser-native activations for the shell's normal Fission action
    /// dispatcher. Call this on the event-loop thread after a wake event.
    pub(crate) fn drain_events(&self) -> Vec<WebAccessibilityEvent> {
        self.events.borrow_mut().drain(..).collect()
    }

    fn update_mounted_node(
        &self,
        mounted: &mut MountedNode,
        description: &NodeDescription,
    ) -> Result<(), String> {
        for attribute in mounted.attributes.keys() {
            if !description.attributes.contains_key(attribute) {
                mounted
                    .element
                    .remove_attribute(attribute)
                    .map_err(crate::js_error_to_string)?;
            }
        }
        for (attribute, value) in &description.attributes {
            if mounted.attributes.get(attribute) != Some(value) {
                mounted
                    .element
                    .set_attribute(attribute, value)
                    .map_err(crate::js_error_to_string)?;
            }
        }
        if mounted.text_content != description.text_content {
            mounted
                .element
                .set_text_content(description.text_content.as_deref());
        }
        sync_native_properties(
            &mounted.element,
            description,
            mounted.native_value.as_deref(),
            mounted.attributes.contains_key("checked"),
        )?;
        update_activation_listener(
            mounted,
            description.intercept_click,
            &self.events,
            &self.proxy,
        )?;
        update_focus_listeners(
            mounted,
            is_focusable_description(description),
            &self.events,
            &self.proxy,
            &self.suppress_focus_events,
        )?;
        update_native_input_listeners(
            mounted,
            description.captures_native_input,
            &self.events,
            &self.proxy,
        )?;
        mounted.parent = description.parent;
        mounted.attributes.clone_from(&description.attributes);
        mounted.text_content.clone_from(&description.text_content);
        mounted.native_value.clone_from(&description.native_value);
        Ok(())
    }

    fn reconcile_children(
        &self,
        parent: Option<WidgetId>,
        child_order: &HashMap<Option<WidgetId>, Vec<WidgetId>>,
    ) -> Result<(), String> {
        let parent_element = match parent {
            Some(parent) => self
                .nodes
                .get(&parent)
                .map(|node| node.element.clone())
                .ok_or_else(|| "semantic parent is missing from the DOM projection".to_string())?,
            None => self.root.clone().unchecked_into::<Element>(),
        };
        let Some(children) = child_order.get(&parent) else {
            return Ok(());
        };
        for (index, widget_id) in children.iter().enumerate() {
            let child = &self
                .nodes
                .get(widget_id)
                .ok_or_else(|| "semantic child is missing from the DOM projection".to_string())?
                .element;
            let at_index = child_at(&parent_element, index);
            if at_index
                .as_ref()
                .is_some_and(|node| node.is_same_node(Some(child.as_ref())))
            {
                continue;
            }
            if let Some(reference) = at_index {
                parent_element
                    .insert_before(child.as_ref(), Some(&reference))
                    .map_err(crate::js_error_to_string)?;
            } else {
                parent_element
                    .append_child(child.as_ref())
                    .map_err(crate::js_error_to_string)?;
            }
        }
        Ok(())
    }
}

impl Drop for WebAccessibilityProjection {
    fn drop(&mut self) {
        for (_, mounted) in std::mem::take(&mut self.nodes) {
            let _ = remove_mounted_node(mounted);
        }
        if let Some(parent) = self.root.parent_node() {
            let _ = parent.remove_child(&self.root);
        }
        let _ = self
            .root
            .remove_event_listener_with_callback("keydown", self.keydown.as_ref().unchecked_ref());
        restore_attribute(
            &self.canvas,
            "aria-hidden",
            self.original_canvas_aria_hidden.as_deref(),
        );
        restore_attribute(&self.canvas, "role", self.original_canvas_role.as_deref());
    }
}

fn restore_attribute(element: &Element, name: &str, value: Option<&str>) {
    match value {
        Some(value) => {
            let _ = element.set_attribute(name, value);
        }
        None => {
            let _ = element.remove_attribute(name);
        }
    }
}

fn remove_mounted_node(mut mounted: MountedNode) -> Result<(), String> {
    if let Some(listener) = mounted.activation.take() {
        mounted
            .element
            .remove_event_listener_with_callback(
                "click",
                listener.callback.as_ref().unchecked_ref(),
            )
            .map_err(crate::js_error_to_string)?;
    }
    if let Some(listeners) = mounted.focus.take() {
        mounted
            .element
            .remove_event_listener_with_callback(
                "focusin",
                listeners.focus_in.as_ref().unchecked_ref(),
            )
            .map_err(crate::js_error_to_string)?;
        mounted
            .element
            .remove_event_listener_with_callback(
                "focusout",
                listeners.focus_out.as_ref().unchecked_ref(),
            )
            .map_err(crate::js_error_to_string)?;
    }
    if let Some(listeners) = mounted.native_input.take() {
        mounted
            .element
            .remove_event_listener_with_callback("input", listeners.input.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
    }
    if let Some(parent) = mounted.element.parent_node() {
        parent
            .remove_child(&mounted.element)
            .map_err(crate::js_error_to_string)?;
    }
    Ok(())
}

fn create_element(
    document: &web_sys::Document,
    description: &NodeDescription,
) -> Result<Element, String> {
    document
        .create_element(description.tag)
        .map_err(crate::js_error_to_string)
}

fn mount_node(
    element: Element,
    description: &NodeDescription,
    events: &Rc<std::cell::RefCell<VecDeque<WebAccessibilityEvent>>>,
    proxy: &EventLoopProxy<TestEvent>,
    suppress_focus_events: &Rc<Cell<bool>>,
) -> Result<MountedNode, String> {
    for (attribute, value) in &description.attributes {
        element
            .set_attribute(attribute, value)
            .map_err(crate::js_error_to_string)?;
    }
    if let Some(text) = &description.text_content {
        element.set_text_content(Some(text));
    }
    let mut mounted = MountedNode {
        widget_id: description.widget_id,
        element,
        tag: description.tag,
        parent: description.parent,
        attributes: description.attributes.clone(),
        text_content: description.text_content.clone(),
        native_value: description.native_value.clone(),
        activation: None,
        focus: None,
        native_input: None,
    };
    sync_native_properties(&mounted.element, description, None, false)?;
    update_activation_listener(&mut mounted, description.intercept_click, events, proxy)?;
    update_focus_listeners(
        &mut mounted,
        is_focusable_description(description),
        events,
        proxy,
        suppress_focus_events,
    )?;
    update_native_input_listeners(
        &mut mounted,
        description.captures_native_input,
        events,
        proxy,
    )?;
    Ok(mounted)
}

fn update_native_input_listeners(
    mounted: &mut MountedNode,
    enabled: bool,
    events: &Rc<std::cell::RefCell<VecDeque<WebAccessibilityEvent>>>,
    proxy: &EventLoopProxy<TestEvent>,
) -> Result<(), String> {
    match (mounted.native_input.as_ref(), enabled) {
        (Some(_), true) | (None, false) => return Ok(()),
        _ => {}
    }
    if let Some(listeners) = mounted.native_input.take() {
        mounted
            .element
            .remove_event_listener_with_callback("input", listeners.input.as_ref().unchecked_ref())
            .map_err(crate::js_error_to_string)?;
    }
    if !enabled {
        return Ok(());
    }

    let events_for_input = events.clone();
    let proxy_for_input = proxy.clone();
    let element_for_input = mounted.element.clone();
    let widget_id = mounted.widget_id;
    let input = Closure::wrap(Box::new(move |event: InputEvent| {
        let Some(value) = read_native_input_value(
            widget_id,
            &element_for_input,
            event.input_type(),
            event.is_composing(),
        ) else {
            return;
        };
        events_for_input
            .borrow_mut()
            .push_back(WebAccessibilityEvent::NativeInput(value));
        crate::send_wake_event(&proxy_for_input);
    }) as Box<dyn FnMut(_)>);
    mounted
        .element
        .add_event_listener_with_callback("input", input.as_ref().unchecked_ref())
        .map_err(crate::js_error_to_string)?;
    mounted.native_input = Some(NativeInputListeners { input });
    Ok(())
}

fn read_native_input_value(
    widget_id: WidgetId,
    element: &Element,
    input_type: String,
    is_composing: bool,
) -> Option<NativeInputValue> {
    let (value, selection_start, selection_end, selection_direction) =
        match element.tag_name().as_str() {
            "INPUT" => {
                let input = element.clone().dyn_into::<HtmlInputElement>().ok()?;
                (
                    input.value(),
                    input.selection_start().ok().flatten(),
                    input.selection_end().ok().flatten(),
                    input.selection_direction().ok().flatten(),
                )
            }
            "TEXTAREA" => {
                let textarea = element.clone().dyn_into::<HtmlTextAreaElement>().ok()?;
                (
                    textarea.value(),
                    textarea.selection_start().ok().flatten(),
                    textarea.selection_end().ok().flatten(),
                    textarea.selection_direction().ok().flatten(),
                )
            }
            _ => return None,
        };
    Some(NativeInputValue {
        widget_id,
        value,
        selection_start,
        selection_end,
        selection_direction,
        input_type,
        is_composing,
    })
}

fn update_activation_listener(
    mounted: &mut MountedNode,
    intercept: Option<bool>,
    events: &Rc<std::cell::RefCell<VecDeque<WebAccessibilityEvent>>>,
    proxy: &EventLoopProxy<TestEvent>,
) -> Result<(), String> {
    match (mounted.activation.as_mut(), intercept) {
        (Some(listener), Some(intercept)) => listener.intercept.set(intercept),
        (Some(_), None) => {
            let listener = mounted.activation.take().expect("listener was present");
            mounted
                .element
                .remove_event_listener_with_callback(
                    "click",
                    listener.callback.as_ref().unchecked_ref(),
                )
                .map_err(crate::js_error_to_string)?;
        }
        (None, Some(intercept)) => {
            let enabled = Rc::new(Cell::new(intercept));
            let enabled_for_handler = enabled.clone();
            let events = events.clone();
            let proxy = proxy.clone();
            let widget_id = mounted.widget_id;
            let callback = Closure::wrap(Box::new(move |event: MouseEvent| {
                if !enabled_for_handler.get()
                    || event.meta_key()
                    || event.ctrl_key()
                    || event.shift_key()
                    || event.alt_key()
                {
                    return;
                }
                event.prevent_default();
                events
                    .borrow_mut()
                    .push_back(WebAccessibilityEvent::Activate(widget_id));
                crate::send_wake_event(&proxy);
            }) as Box<dyn FnMut(_)>);
            mounted
                .element
                .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())
                .map_err(crate::js_error_to_string)?;
            mounted.activation = Some(ActivationListener {
                intercept: enabled,
                callback,
            });
        }
        (None, None) => {}
    }
    Ok(())
}

fn is_focusable_description(description: &NodeDescription) -> bool {
    description.attributes.contains_key("tabindex")
        || match description.tag {
            "a" => description.attributes.contains_key("href"),
            "button" | "input" | "textarea" => !description.attributes.contains_key("disabled"),
            _ => false,
        }
}

fn update_focus_listeners(
    mounted: &mut MountedNode,
    enabled: bool,
    events: &Rc<std::cell::RefCell<VecDeque<WebAccessibilityEvent>>>,
    proxy: &EventLoopProxy<TestEvent>,
    suppress_focus_events: &Rc<Cell<bool>>,
) -> Result<(), String> {
    match (mounted.focus.as_mut(), enabled) {
        (Some(_), true) => {}
        (Some(_), false) => {
            let listeners = mounted.focus.take().expect("focus listeners were present");
            mounted
                .element
                .remove_event_listener_with_callback(
                    "focusin",
                    listeners.focus_in.as_ref().unchecked_ref(),
                )
                .map_err(crate::js_error_to_string)?;
            mounted
                .element
                .remove_event_listener_with_callback(
                    "focusout",
                    listeners.focus_out.as_ref().unchecked_ref(),
                )
                .map_err(crate::js_error_to_string)?;
        }
        (None, true) => {
            let element_for_focus_in = mounted.element.clone();
            let element_for_focus_out = mounted.element.clone();
            let events_for_focus_in = events.clone();
            let events_for_focus_out = events.clone();
            let proxy_for_focus_in = proxy.clone();
            let proxy_for_focus_out = proxy.clone();
            let suppress_for_focus_in = suppress_focus_events.clone();
            let suppress_for_focus_out = suppress_focus_events.clone();
            let widget_id = mounted.widget_id;
            let focus_in = Closure::wrap(Box::new(move |event: Event| {
                if suppress_for_focus_in.get()
                    || !event
                        .target()
                        .and_then(|target| target.dyn_into::<Element>().ok())
                        .is_some_and(|target| target.is_same_node(Some(&element_for_focus_in)))
                {
                    return;
                }
                events_for_focus_in
                    .borrow_mut()
                    .push_back(WebAccessibilityEvent::Focus(widget_id));
                crate::send_wake_event(&proxy_for_focus_in);
            }) as Box<dyn FnMut(_)>);
            let widget_id = mounted.widget_id;
            let focus_out = Closure::wrap(Box::new(move |event: Event| {
                if suppress_for_focus_out.get()
                    || !event
                        .target()
                        .and_then(|target| target.dyn_into::<Element>().ok())
                        .is_some_and(|target| target.is_same_node(Some(&element_for_focus_out)))
                {
                    return;
                }
                events_for_focus_out
                    .borrow_mut()
                    .push_back(WebAccessibilityEvent::Blur(widget_id));
                crate::send_wake_event(&proxy_for_focus_out);
            }) as Box<dyn FnMut(_)>);
            mounted
                .element
                .add_event_listener_with_callback("focusin", focus_in.as_ref().unchecked_ref())
                .map_err(crate::js_error_to_string)?;
            mounted
                .element
                .add_event_listener_with_callback("focusout", focus_out.as_ref().unchecked_ref())
                .map_err(crate::js_error_to_string)?;
            mounted.focus = Some(FocusListeners {
                focus_in,
                focus_out,
            });
        }
        (None, false) => {}
    }
    Ok(())
}

fn child_at(parent: &Element, index: usize) -> Option<web_sys::Node> {
    let mut current = parent.first_child();
    for _ in 0..index {
        current = current?.next_sibling();
    }
    current
}

fn ordered_widget_ids(ir: &CoreIR, available: &HashSet<WidgetId>) -> Vec<WidgetId> {
    fn visit(
        ir: &CoreIR,
        available: &HashSet<WidgetId>,
        current: WidgetId,
        visited: &mut HashSet<WidgetId>,
        ordered: &mut Vec<WidgetId>,
    ) {
        if !visited.insert(current) {
            return;
        }
        if available.contains(&current) {
            ordered.push(current);
        }
        if let Some(node) = ir.nodes.get(&current) {
            for child in &node.children {
                visit(ir, available, *child, visited, ordered);
            }
        }
    }

    let mut visited = HashSet::new();
    let mut ordered = Vec::with_capacity(available.len());
    if let Some(root) = ir.root {
        visit(ir, available, root, &mut visited, &mut ordered);
    }
    // A malformed or partially built IR can contain detached semantic nodes.
    // Retain them deterministically after the rooted tree instead of dropping
    // a control from the browser's accessibility view.
    let mut detached = available
        .iter()
        .filter(|widget_id| !visited.contains(widget_id))
        .copied()
        .collect::<Vec<_>>();
    detached.sort_by_key(|widget_id| widget_id.as_u128());
    for widget_id in detached {
        visit(ir, available, widget_id, &mut visited, &mut ordered);
    }
    ordered
}

fn nearest_projected_parent(
    ir: &CoreIR,
    widget_id: WidgetId,
    projected: &HashSet<WidgetId>,
) -> Option<WidgetId> {
    let mut current = ir.nodes.get(&widget_id).and_then(|node| node.parent);
    while let Some(parent) = current {
        if projected.contains(&parent) {
            return Some(parent);
        }
        current = ir.nodes.get(&parent).and_then(|node| node.parent);
    }
    None
}

fn describe_semantics(
    widget_id: WidgetId,
    semantics: &Semantics,
    fallback_name: Option<String>,
    active_ids: &HashSet<WidgetId>,
) -> NodeDescription {
    let heading = site_heading_level(semantics.identifier.as_deref());
    let landmark = site_landmark(semantics.identifier.as_deref());
    let is_link = semantics.hyperlink.is_some();
    let tag = heading
        .map(heading_tag)
        .or_else(|| landmark.map(|landmark| landmark.tag))
        .unwrap_or_else(|| semantic_tag(semantics, is_link));

    let mut attributes = BTreeMap::new();
    attributes.insert("id".to_owned(), dom_id(widget_id));
    attributes.insert("data-fission-widget-id".to_owned(), widget_id.to_string());
    if let Some(identifier) = semantics.identifier.as_deref() {
        attributes.insert("data-fission-semantics".to_owned(), identifier.to_owned());
    }
    if let Some(landmark) = landmark {
        attributes.insert("role".to_owned(), landmark.role.to_owned());
    } else if let Some(role) = semantic_role_attribute(semantics, is_link && !semantics.disabled) {
        attributes.insert("role".to_owned(), role.to_owned());
    }
    if tag == "button" {
        attributes.insert("type".to_owned(), "button".to_owned());
    }

    let name = nonempty(semantics.label.as_deref()).or(fallback_name.as_deref());
    let labelled_by = live_widget_ids(&semantics.labelled_by, active_ids);
    if labelled_by.is_empty() {
        match semantics.role {
            Role::Image => {
                attributes.insert("alt".to_owned(), name.unwrap_or_default().to_owned());
            }
            Role::Text => {}
            _ => {
                if let Some(name) = name {
                    attributes.insert("aria-label".to_owned(), name.to_owned());
                }
            }
        }
    } else {
        attributes.insert(
            "aria-labelledby".to_owned(),
            widget_ids_attribute(&labelled_by),
        );
    }

    insert_widget_relation(
        &mut attributes,
        "aria-describedby",
        &live_widget_ids(&semantics.described_by, active_ids),
    );
    insert_widget_relation(
        &mut attributes,
        "aria-controls",
        &live_widget_ids(&semantics.controls, active_ids),
    );
    if let Some(active) = semantics
        .active_descendant
        .filter(|active| active_ids.contains(active))
    {
        attributes.insert("aria-activedescendant".to_owned(), dom_id(active));
    }
    if let Some(checked) = semantics.checked {
        attributes.insert("aria-checked".to_owned(), checked.to_string());
        if checked {
            attributes.insert("checked".to_owned(), String::new());
        }
    }
    if let Some(selected) = semantics.selected {
        attributes.insert("aria-selected".to_owned(), selected.to_string());
    }
    if let Some(expanded) = semantics.expanded {
        attributes.insert("aria-expanded".to_owned(), expanded.to_string());
    }
    attributes.insert("aria-disabled".to_owned(), semantics.disabled.to_string());
    if semantics.disabled && is_native_disabled_control(tag) {
        attributes.insert("disabled".to_owned(), String::new());
    }
    if semantics.read_only {
        attributes.insert("aria-readonly".to_owned(), "true".to_owned());
        if matches!(tag, "input" | "textarea") {
            attributes.insert("readonly".to_owned(), String::new());
        }
    }
    if semantics.required {
        attributes.insert("aria-required".to_owned(), "true".to_owned());
        if matches!(tag, "input" | "textarea") {
            attributes.insert("required".to_owned(), String::new());
        }
    }
    if semantics.validation_state == TextFieldValidationState::Invalid {
        attributes.insert("aria-invalid".to_owned(), "true".to_owned());
    }
    if let Some(message) = nonempty(semantics.validation_message.as_deref()) {
        attributes.insert("aria-description".to_owned(), message.to_owned());
    }
    if semantics.busy {
        attributes.insert("aria-busy".to_owned(), "true".to_owned());
    }
    if semantics.modal {
        attributes.insert("aria-modal".to_owned(), "true".to_owned());
    }
    if let Some(popup) = semantics.has_popup {
        attributes.insert("aria-haspopup".to_owned(), popup_name(popup).to_owned());
    }
    if let Some(orientation) = semantics.orientation {
        attributes.insert(
            "aria-orientation".to_owned(),
            match orientation {
                fission_ir::SemanticOrientation::Horizontal => "horizontal",
                fission_ir::SemanticOrientation::Vertical => "vertical",
            }
            .to_owned(),
        );
    }
    if let Some(minimum) = semantics.min_value.filter(|value| value.is_finite()) {
        attributes.insert("aria-valuemin".to_owned(), minimum.to_string());
        if matches!(tag, "input" | "progress") {
            attributes.insert("min".to_owned(), minimum.to_string());
        }
    }
    if let Some(maximum) = semantics.max_value.filter(|value| value.is_finite()) {
        attributes.insert("aria-valuemax".to_owned(), maximum.to_string());
        if matches!(tag, "input" | "progress") {
            attributes.insert("max".to_owned(), maximum.to_string());
        }
    }
    if let Some(current) = semantics.current_value.filter(|value| value.is_finite()) {
        attributes.insert("aria-valuenow".to_owned(), current.to_string());
        if matches!(tag, "input" | "progress") {
            attributes.insert("value".to_owned(), current.to_string());
        }
    }
    if semantics.scrollable_x || semantics.scrollable_y {
        attributes.insert("tabindex".to_owned(), "0".to_owned());
    } else if semantics.is_sequentially_focusable() {
        attributes.insert("tabindex".to_owned(), "0".to_owned());
    } else if semantics.focusable && !semantics.disabled {
        attributes.insert("tabindex".to_owned(), "-1".to_owned());
    }
    if let Some(focus_index) = semantics.focus_index {
        attributes.insert(
            "data-fission-focus-index".to_owned(),
            focus_index.to_string(),
        );
    }
    if semantics.is_focus_barrier {
        attributes.insert("data-fission-focus-barrier".to_owned(), "true".to_owned());
    }
    if let Some(name) = semantics.text_field_name.as_deref() {
        attributes.insert("name".to_owned(), name.to_owned());
    }

    if matches!(tag, "input") {
        attributes.insert("type".to_owned(), input_type(semantics).to_owned());
        if !semantics.masked {
            if let Some(value) = semantics.value.as_deref() {
                attributes.insert("value".to_owned(), value.to_owned());
            }
        }
    }
    if semantics.role == Role::Status {
        attributes.insert("aria-live".to_owned(), "polite".to_owned());
        attributes.insert("aria-atomic".to_owned(), "true".to_owned());
    } else if semantics.role == Role::Alert {
        attributes.insert("aria-live".to_owned(), "assertive".to_owned());
        attributes.insert("aria-atomic".to_owned(), "true".to_owned());
    }
    if is_link && !semantics.disabled {
        add_link_attributes(&mut attributes, semantics);
    }

    let text_content = if semantics.role == Role::Text && !semantics.masked {
        nonempty(semantics.label.as_deref())
            .or_else(|| nonempty(semantics.value.as_deref()))
            .or(fallback_name.as_deref())
            .map(str::to_owned)
    } else if tag == "textarea" && !semantics.masked {
        semantics.value.clone()
    } else {
        None
    };
    let native_value = if matches!(tag, "input" | "textarea") {
        semantics
            .value
            .clone()
            .or_else(|| semantics.current_value.map(|value| value.to_string()))
            .or_else(|| Some(String::new()))
    } else {
        None
    };
    let intercept_click = if semantics.disabled {
        None
    } else if let Some(hyperlink) = semantics.hyperlink.as_ref() {
        (matches!(hyperlink.target, LinkTarget::Current)
            && hyperlink.download.is_none()
            && semantics.popover_target.is_none())
        .then_some(true)
    } else {
        semantics
            .actions
            .entries
            .iter()
            .any(|entry| entry.trigger == ActionTrigger::Default)
            .then_some(true)
    };

    NodeDescription {
        widget_id,
        parent: None,
        tag,
        attributes,
        text_content,
        native_value,
        intercept_click,
        pointer_interactive: is_link || tag == "button" || matches!(tag, "input" | "textarea"),
        captures_native_input: matches!(tag, "input" | "textarea"),
    }
}

fn semantic_tag(semantics: &Semantics, has_hyperlink: bool) -> &'static str {
    if has_hyperlink {
        return "a";
    }
    match semantics.role {
        Role::Button | Role::Tab => "button",
        Role::Link => "button",
        Role::MenuItem => "button",
        Role::Text => "span",
        Role::TextInput | Role::Input | Role::ComboBox
            if !semantics.masked
                && (semantics.text_input_type == TextInputType::Multiline
                    || semantics.multiline) =>
        {
            "textarea"
        }
        Role::TextInput | Role::Input | Role::ComboBox | Role::SpinButton | Role::Slider => "input",
        Role::Checkbox | Role::Radio | Role::Switch => "input",
        Role::Image => "img",
        Role::Video => "video",
        Role::Generic
            if semantics
                .actions
                .entries
                .iter()
                .any(|entry| entry.trigger == ActionTrigger::Default) =>
        {
            "button"
        }
        Role::Dialog
        | Role::Generic
        | Role::Menu
        | Role::ListBox
        | Role::Option
        | Role::TabList
        | Role::TabPanel
        | Role::Alert
        | Role::Group
        | Role::Tree
        | Role::TreeItem
        | Role::Toolbar
        | Role::RadioGroup
        | Role::Table
        | Role::TableRow
        | Role::TableCell
        | Role::ColumnHeader
        | Role::Tooltip
        | Role::Status => "div",
        Role::List => "ul",
        Role::ListItem => "li",
        Role::Separator => "hr",
        Role::ProgressBar => "progress",
    }
}

fn semantic_role_attribute(semantics: &Semantics, has_hyperlink: bool) -> Option<&'static str> {
    match semantics.role {
        Role::Link if !has_hyperlink => Some("link"),
        Role::MenuItem => Some("menuitem"),
        Role::Switch => Some("switch"),
        Role::Dialog => Some("dialog"),
        Role::Menu => Some("menu"),
        Role::ListBox => Some("listbox"),
        Role::Option => Some("option"),
        Role::ComboBox => Some("combobox"),
        Role::TabList => Some("tablist"),
        Role::Tab => Some("tab"),
        Role::TabPanel => Some("tabpanel"),
        Role::Alert => Some("alert"),
        Role::Group => Some("group"),
        Role::Separator => Some("separator"),
        Role::Tree => Some("tree"),
        Role::TreeItem => Some("treeitem"),
        Role::Toolbar => Some("toolbar"),
        Role::RadioGroup => Some("radiogroup"),
        Role::Table => Some("table"),
        Role::TableRow => Some("row"),
        Role::TableCell => Some("cell"),
        Role::ColumnHeader => Some("columnheader"),
        Role::ProgressBar => Some("progressbar"),
        Role::Tooltip => Some("tooltip"),
        Role::Status => Some("status"),
        _ => None,
    }
}

fn site_heading_level(identifier: Option<&str>) -> Option<u8> {
    let value = identifier?.strip_prefix("site-heading-")?;
    let (level, suffix) = value.split_once(':')?;
    if suffix.is_empty() {
        return None;
    }
    match level.parse::<u8>().ok()? {
        level @ 1..=6 => Some(level),
        _ => None,
    }
}

fn heading_tag(level: u8) -> &'static str {
    match level {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        6 => "h6",
        _ => "div",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Landmark {
    tag: &'static str,
    role: &'static str,
}

fn site_landmark(identifier: Option<&str>) -> Option<Landmark> {
    let suffix = identifier?.strip_prefix("site-landmark-")?;
    let suffix = suffix.split(':').next()?;
    let landmark = match suffix {
        "banner" => Landmark {
            tag: "header",
            role: "banner",
        },
        "main" => Landmark {
            tag: "main",
            role: "main",
        },
        "navigation" => Landmark {
            tag: "nav",
            role: "navigation",
        },
        "complementary" => Landmark {
            tag: "aside",
            role: "complementary",
        },
        "contentinfo" => Landmark {
            tag: "footer",
            role: "contentinfo",
        },
        "search" => Landmark {
            tag: "div",
            role: "search",
        },
        "form" => Landmark {
            tag: "div",
            role: "form",
        },
        "region" => Landmark {
            tag: "section",
            role: "region",
        },
        _ => return None,
    };
    Some(landmark)
}

fn input_type(semantics: &Semantics) -> &'static str {
    match semantics.role {
        Role::Checkbox | Role::Switch => "checkbox",
        Role::Radio => "radio",
        Role::Slider => "range",
        Role::SpinButton => "number",
        _ if semantics.masked => "password",
        _ => match semantics.text_input_type {
            TextInputType::Number => "number",
            TextInputType::EmailAddress => "email",
            TextInputType::Url => "url",
            TextInputType::Phone => "tel",
            TextInputType::Text | TextInputType::Multiline | TextInputType::Name => "text",
        },
    }
}

fn popup_name(popup: fission_ir::PopupKind) -> &'static str {
    match popup {
        fission_ir::PopupKind::Menu => "menu",
        fission_ir::PopupKind::ListBox => "listbox",
        fission_ir::PopupKind::Tree => "tree",
        fission_ir::PopupKind::Grid => "grid",
        fission_ir::PopupKind::Dialog => "dialog",
    }
}

fn is_native_disabled_control(tag: &str) -> bool {
    matches!(tag, "button" | "input" | "textarea" | "select" | "progress")
}

fn dom_id(widget_id: WidgetId) -> String {
    format!("fission-sem-{:032x}", widget_id.as_u128())
}

fn widget_ids_attribute(ids: &[WidgetId]) -> String {
    ids.iter()
        .map(|widget_id| dom_id(*widget_id))
        .collect::<Vec<_>>()
        .join(" ")
}

fn live_widget_ids(ids: &[WidgetId], active_ids: &HashSet<WidgetId>) -> Vec<WidgetId> {
    ids.iter()
        .copied()
        .filter(|widget_id| active_ids.contains(widget_id))
        .collect()
}

fn insert_widget_relation(attributes: &mut BTreeMap<String, String>, name: &str, ids: &[WidgetId]) {
    if !ids.is_empty() {
        attributes.insert(name.to_owned(), widget_ids_attribute(ids));
    }
}

fn add_link_attributes(attributes: &mut BTreeMap<String, String>, semantics: &Semantics) {
    if semantics.disabled {
        return;
    }
    let Some(link) = semantics.hyperlink.as_ref() else {
        return;
    };
    attributes.insert("href".to_owned(), link.href.clone());
    attributes.insert("target".to_owned(), link.target.as_html_target().to_owned());
    if let Some(rel) = link.rel.as_deref() {
        attributes.insert("rel".to_owned(), rel.to_owned());
    } else if matches!(link.target, LinkTarget::NewWindow) {
        attributes.insert("rel".to_owned(), "noopener noreferrer".to_owned());
    }
    if let Some(download) = link.download.as_deref() {
        attributes.insert("download".to_owned(), download.to_owned());
    }
    if let Some(popover) = semantics.popover_target.as_ref() {
        attributes.insert("popovertarget".to_owned(), popover.id.clone());
        attributes.insert(
            "popovertargetaction".to_owned(),
            popover.action.as_html_action().to_owned(),
        );
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

fn should_expose_semantics(semantics: &Semantics) -> bool {
    semantics.role != Role::Generic
        || semantics.label.is_some()
        || semantics.identifier.is_some()
        || semantics.value.is_some()
        || semantics.focusable
        || semantics.text_editable
        || semantics.checked.is_some()
        || semantics.selected.is_some()
        || semantics.expanded.is_some()
        || semantics.has_popup.is_some()
        || semantics.orientation.is_some()
        || semantics.modal
        || semantics.busy
        || semantics.required
        || semantics.validation_state == TextFieldValidationState::Invalid
        || semantics.validation_message.is_some()
        || semantics.current_value.is_some()
        || semantics.scrollable_x
        || semantics.scrollable_y
        || !semantics.controls.is_empty()
        || !semantics.labelled_by.is_empty()
        || !semantics.described_by.is_empty()
        || semantics.active_descendant.is_some()
        || semantics.hyperlink.is_some()
        || !semantics.actions.entries.is_empty()
}

fn fallback_accessible_name(
    ir: &CoreIR,
    widget_id: WidgetId,
    semantics: &Semantics,
) -> Option<String> {
    let role_has_content_name = matches!(
        semantics.role,
        Role::Button
            | Role::Link
            | Role::MenuItem
            | Role::Image
            | Role::Video
            | Role::TextInput
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Slider
            | Role::Input
            | Role::Option
            | Role::ComboBox
            | Role::Tab
            | Role::Text
    );
    let identified_section = semantics.identifier.as_deref().is_some_and(|identifier| {
        site_heading_level(Some(identifier)).is_some() || site_landmark(Some(identifier)).is_some()
    });
    if semantics.masked || (!role_has_content_name && !identified_section) {
        return None;
    }
    let mut text = String::new();
    collect_descendant_text(ir, widget_id, &mut text);
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn collect_descendant_text(ir: &CoreIR, widget_id: WidgetId, out: &mut String) {
    let Some(node) = ir.nodes.get(&widget_id) else {
        return;
    };
    match &node.op {
        Op::Paint(PaintOp::DrawText { text, .. }) => append_text(out, text),
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
            append_text(out, &text);
        }
        _ => {
            for child in &node.children {
                collect_descendant_text(ir, *child, out);
            }
        }
    }
}

fn append_text(out: &mut String, text: &str) {
    if !text.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(text);
    }
}

fn geometry_style(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    canvas_left: f64,
    canvas_top: f64,
    scale_x: f64,
    scale_y: f64,
    pointer_interactive: bool,
) -> String {
    let pointer_events = if pointer_interactive { "auto" } else { "none" };
    format!(
        "position:absolute;left:{:.3}px;top:{:.3}px;width:{:.3}px;height:{:.3}px;display:block;box-sizing:border-box;margin:0;padding:0;border:0;appearance:none;pointer-events:{};color:transparent;background:transparent;caret-color:transparent;outline:none;box-shadow:none;font:inherit;line-height:inherit;white-space:pre-wrap;",
        canvas_left + x * scale_x,
        canvas_top + y * scale_y,
        width * scale_x,
        height * scale_y,
        pointer_events,
    )
}

fn sync_native_properties(
    element: &Element,
    description: &NodeDescription,
    previous_native_value: Option<&str>,
    previous_checked: bool,
) -> Result<(), String> {
    match description.tag {
        "input" => {
            let input = element
                .clone()
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "semantic input DOM node has the wrong element type".to_string())?;
            if description.native_value.as_deref() != previous_native_value
                && let Some(value) = description.native_value.as_deref()
                && input.value() != value
            {
                input.set_value(value);
            }
            let checked = description.attributes.contains_key("checked");
            if checked != previous_checked {
                input.set_checked(checked);
            }
        }
        "textarea" => {
            let textarea = element
                .clone()
                .dyn_into::<HtmlTextAreaElement>()
                .map_err(|_| "semantic textarea DOM node has the wrong element type".to_string())?;
            if description.native_value.as_deref() != previous_native_value
                && let Some(value) = description.native_value.as_deref()
                && textarea.value() != value
            {
                textarea.set_value(value);
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        describe_semantics, input_type, live_widget_ids, semantic_role_attribute, semantic_tag,
    };
    use fission_ir::{
        ActionEntry, ActionSet, ActionTrigger, Role, Semantics, TextInputType, WidgetId,
    };
    use std::collections::HashSet;

    #[test]
    fn role_link_without_href_uses_keyboard_activatable_button() {
        let semantics = Semantics {
            role: Role::Link,
            ..Semantics::default()
        };

        assert_eq!(semantic_tag(&semantics, false), "button");
        assert_eq!(semantic_role_attribute(&semantics, false), Some("link"));
    }

    #[test]
    fn action_button_is_non_submitting_and_pointer_interactive() {
        let widget_id = WidgetId::from_u128(5);
        let semantics = Semantics {
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: 9,
                    payload_data: None,
                }],
            },
            ..Semantics::default()
        };
        let description = describe_semantics(
            widget_id,
            &semantics,
            Some("Open details".to_owned()),
            &HashSet::from([widget_id]),
        );

        assert_eq!(description.tag, "button");
        assert_eq!(
            description.attributes.get("type").map(String::as_str),
            Some("button")
        );
        assert!(description.pointer_interactive);
    }

    #[test]
    fn masked_multiline_input_uses_password_control() {
        let semantics = Semantics {
            role: Role::TextInput,
            text_input_type: TextInputType::Multiline,
            multiline: true,
            masked: true,
            ..Semantics::default()
        };

        assert_eq!(semantic_tag(&semantics, false), "input");
        assert_eq!(input_type(&semantics), "password");
    }

    #[test]
    fn relationships_include_only_active_semantic_nodes() {
        let active = WidgetId::from_u128(2);
        let stale = WidgetId::from_u128(3);
        let active_ids = HashSet::from([active]);

        assert_eq!(live_widget_ids(&[active, stale], &active_ids), vec![active]);
    }
}
