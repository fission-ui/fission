use std::cell::RefCell;

use fission_ir::semantics::{TextCapitalization, TextInputAction, TextInputType};
use fission_ir::WidgetId;
use fission_layout::LayoutSize;
use fission_render::LayoutRect;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlElement, HtmlInputElement, HtmlTextAreaElement};

use super::{byte_to_utf16, TextInputConfig};

#[derive(Clone)]
pub(crate) enum WebTextControl {
    Input(HtmlInputElement),
    Textarea(HtmlTextAreaElement),
}

impl WebTextControl {
    pub(crate) fn html_element(&self) -> HtmlElement {
        match self {
            Self::Input(control) => control.clone().unchecked_into(),
            Self::Textarea(control) => control.clone().unchecked_into(),
        }
    }

    pub(crate) fn value(&self) -> String {
        match self {
            Self::Input(control) => control.value(),
            Self::Textarea(control) => control.value(),
        }
    }

    pub(crate) fn set_value(&self, value: &str) {
        match self {
            Self::Input(control) => control.set_value(value),
            Self::Textarea(control) => control.set_value(value),
        }
    }

    pub(crate) fn set_selection_utf16(&self, anchor: usize, caret: usize) {
        let start = u32::try_from(anchor.min(caret)).unwrap_or(u32::MAX);
        let end = u32::try_from(anchor.max(caret)).unwrap_or(u32::MAX);
        let direction = if anchor <= caret {
            "forward"
        } else {
            "backward"
        };
        match self {
            Self::Input(control) => {
                let _ = control.set_selection_range_with_direction(start, end, direction);
            }
            Self::Textarea(control) => {
                let _ = control.set_selection_range_with_direction(start, end, direction);
            }
        }
    }

    pub(crate) fn set_max_length(&self, max_length: Option<usize>) {
        let max_length = max_length.and_then(|length| i32::try_from(length).ok());
        match (self, max_length) {
            (Self::Input(control), Some(max_length)) => control.set_max_length(max_length),
            (Self::Textarea(control), Some(max_length)) => control.set_max_length(max_length),
            (Self::Input(control), None) => {
                let _ = control.remove_attribute("maxlength");
            }
            (Self::Textarea(control), None) => {
                let _ = control.remove_attribute("maxlength");
            }
        }
    }

    pub(crate) fn set_disabled(&self, disabled: bool) {
        match self {
            Self::Input(control) => control.set_disabled(disabled),
            Self::Textarea(control) => control.set_disabled(disabled),
        }
    }

    pub(crate) fn set_read_only(&self, read_only: bool) {
        match self {
            Self::Input(control) => control.set_read_only(read_only),
            Self::Textarea(control) => control.set_read_only(read_only),
        }
    }
}

struct BoundControl {
    widget: WidgetId,
    control: WebTextControl,
    canvas: HtmlCanvasElement,
    viewport: LayoutSize,
}

#[derive(Default)]
struct WebImeState {
    bound: Option<BoundControl>,
    config: Option<TextInputConfig>,
    caret: Option<LayoutRect>,
    allowed: bool,
    composing: bool,
    suspended: bool,
}

thread_local! {
    static WEB_IME_STATE: RefCell<WebImeState> = RefCell::new(WebImeState::default());
}

pub(crate) fn bind_control(
    widget: WidgetId,
    control: WebTextControl,
    canvas: HtmlCanvasElement,
    viewport: LayoutSize,
    config: Option<TextInputConfig>,
) {
    WEB_IME_STATE.with_borrow_mut(|state| {
        let control_element = control.html_element();
        let same_control = state
            .bound
            .as_ref()
            .map(|bound| {
                bound.widget == widget
                    && bound
                        .control
                        .html_element()
                        .is_same_node(Some(&control_element))
            })
            .unwrap_or(false);
        if !same_control {
            if let Some(previous) = state.bound.take() {
                let _ = previous.control.html_element().blur();
            }
        }
        state.bound = Some(BoundControl {
            widget,
            control,
            canvas,
            viewport,
        });
        if !same_control {
            state.composing = false;
        }
        if let Some(config) = config {
            state.config = Some(config);
        }
        apply(state);
    });
}

pub(crate) fn unbind_control(widget: WidgetId) {
    WEB_IME_STATE.with_borrow_mut(|state| {
        if state.bound.as_ref().map(|bound| bound.widget) != Some(widget) {
            return;
        }
        if let Some(bound) = state.bound.take() {
            let _ = bound.control.html_element().blur();
        }
        state.composing = false;
    });
}

pub(crate) fn update_viewport(canvas: HtmlCanvasElement, viewport: LayoutSize) {
    WEB_IME_STATE.with_borrow_mut(|state| {
        if let Some(bound) = state.bound.as_mut() {
            bound.canvas = canvas;
            bound.viewport = viewport;
        }
        apply_caret(state);
    });
}

pub(super) fn sync(allowed: bool, config: Option<TextInputConfig>) {
    WEB_IME_STATE.with_borrow_mut(|state| {
        state.allowed = allowed;
        state.config = config;
        apply(state);
    });
}

pub(super) fn set_caret(caret: LayoutRect) {
    WEB_IME_STATE.with_borrow_mut(|state| {
        state.caret = Some(caret);
        apply_caret(state);
    });
}

pub(crate) fn set_composing(composing: bool) {
    WEB_IME_STATE.with_borrow_mut(|state| {
        state.composing = composing;
    });
}

pub(crate) fn is_composing() -> bool {
    WEB_IME_STATE.with_borrow(|state| state.composing)
}

pub(crate) fn focus_control(widget: WidgetId) -> bool {
    WEB_IME_STATE.with_borrow(|state| {
        if state.suspended || !state.allowed {
            return false;
        }
        let Some(bound) = state.bound.as_ref().filter(|bound| bound.widget == widget) else {
            return false;
        };
        bound.control.html_element().focus().is_ok()
    })
}

pub(crate) fn refresh_geometry() {
    WEB_IME_STATE.with_borrow_mut(apply_caret);
}

pub(crate) fn suspend() {
    WEB_IME_STATE.with_borrow_mut(|state| {
        state.suspended = true;
        state.composing = false;
        if let Some(bound) = state.bound.as_ref() {
            let _ = bound.control.html_element().blur();
        }
    });
}

pub(crate) fn resume() {
    WEB_IME_STATE.with_borrow_mut(|state| {
        state.suspended = false;
        apply(state);
    });
}

pub(super) fn shutdown() {
    WEB_IME_STATE.with_borrow_mut(|state| {
        if let Some(bound) = state.bound.take() {
            let _ = bound.control.html_element().blur();
        }
        *state = WebImeState::default();
    });
}

fn apply(state: &mut WebImeState) {
    let Some(bound) = state.bound.as_ref() else {
        return;
    };
    let active = state.allowed && !state.suspended;
    let element = bound.control.html_element();
    let _ = element.remove_attribute("aria-hidden");
    let _ = element.set_attribute("tabindex", "-1");
    let _ = element.set_attribute("data-fission-ime-control", "true");
    let _ = element.set_attribute("autocomplete", "off");
    if let Some(config) = state.config.as_ref() {
        apply_traits(&bound.control, config);
        if !state.composing {
            if bound.control.value() != config.value {
                bound.control.set_value(&config.value);
            }
            let anchor = byte_to_utf16(&config.value, config.selection.0);
            let caret = byte_to_utf16(&config.value, config.selection.1);
            bound.control.set_selection_utf16(anchor, caret);
        }
    }
    bound.control.set_disabled(!active);
    bound.control.set_read_only(!active);
    apply_hidden_style(&element);
    apply_caret(state);
}

fn apply_traits(control: &WebTextControl, config: &TextInputConfig) {
    let element = control.html_element();
    if let WebTextControl::Input(input) = control {
        input.set_type(if config.masked {
            "password"
        } else {
            match config.text_input_type {
                TextInputType::EmailAddress => "email",
                TextInputType::Url => "url",
                TextInputType::Phone => "tel",
                _ => "text",
            }
        });
    }
    let input_mode = match config.text_input_type {
        TextInputType::Number => "decimal",
        TextInputType::EmailAddress => "email",
        TextInputType::Url => "url",
        TextInputType::Phone => "tel",
        _ => "text",
    };
    let _ = element.set_attribute("inputmode", input_mode);
    let _ = element.set_attribute("enterkeyhint", enter_key_hint(config.text_input_action));
    let _ = element.set_attribute(
        "autocapitalize",
        match config.text_capitalization {
            TextCapitalization::None => "none",
            TextCapitalization::Characters => "characters",
            TextCapitalization::Words => "words",
            TextCapitalization::Sentences => "sentences",
        },
    );
    let _ = element.set_attribute("autocorrect", if config.autocorrect { "on" } else { "off" });
    let _ = element.set_attribute(
        "spellcheck",
        if config.spell_check { "true" } else { "false" },
    );
    let _ = element.set_attribute(
        "data-fission-suggestions",
        if config.enable_suggestions {
            "true"
        } else {
            "false"
        },
    );
    let _ = element.set_attribute(
        "data-fission-smart-dashes",
        if config.smart_dashes { "true" } else { "false" },
    );
    let _ = element.set_attribute(
        "data-fission-smart-quotes",
        if config.smart_quotes { "true" } else { "false" },
    );
    if let Some(hint) = config.autofill_hints.first() {
        let _ = element.set_attribute("autocomplete", hint);
    }
    control.set_max_length(config.max_length);
}

fn enter_key_hint(action: TextInputAction) -> &'static str {
    match action {
        TextInputAction::Done => "done",
        TextInputAction::Go | TextInputAction::Continue | TextInputAction::Join => "go",
        TextInputAction::Search => "search",
        TextInputAction::Send => "send",
        TextInputAction::Next => "next",
        TextInputAction::Previous => "previous",
        TextInputAction::Route => "go",
        TextInputAction::EmergencyCall => "send",
        TextInputAction::Newline => "enter",
    }
}

fn apply_hidden_style(element: &HtmlElement) {
    let style = element.style();
    let _ = style.remove_property("inset");
    for (name, value) in [
        ("position", "fixed"),
        ("left", "0px"),
        ("top", "0px"),
        ("width", "1px"),
        ("height", "1px"),
        ("z-index", "2147483647"),
        ("opacity", "0"),
        ("pointer-events", "none"),
        ("padding", "0"),
        ("margin", "0"),
        ("border", "0"),
        ("outline", "0"),
        ("background", "transparent"),
        ("color", "transparent"),
        ("caret-color", "transparent"),
        ("overflow", "hidden"),
        // Avoid iOS Safari zooming the page when the native editing control
        // receives focus.
        ("font-size", "16px"),
    ] {
        let _ = style.set_property(name, value);
    }
}

fn apply_caret(state: &mut WebImeState) {
    let Some(bound) = state.bound.as_ref() else {
        return;
    };
    let Some(caret) = state.caret else {
        return;
    };
    let canvas_rect = bound.canvas.get_bounding_client_rect();
    let scale_x = safe_scale(canvas_rect.width(), bound.viewport.width);
    let scale_y = safe_scale(canvas_rect.height(), bound.viewport.height);
    let left = canvas_rect.left() + f64::from(caret.x()) * scale_x;
    let top = canvas_rect.top() + f64::from(caret.y()) * scale_y;
    let width = (f64::from(caret.width()) * scale_x).max(1.0);
    let height = (f64::from(caret.height()) * scale_y).max(1.0);
    let style = bound.control.html_element().style();
    let _ = style.set_property("left", &format!("{left}px"));
    let _ = style.set_property("top", &format!("{top}px"));
    let _ = style.set_property("width", &format!("{width}px"));
    let _ = style.set_property("height", &format!("{height}px"));
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

#[cfg(test)]
mod tests {
    use super::safe_scale;

    #[test]
    fn css_scale_tracks_canvas_layout_size() {
        assert_eq!(safe_scale(640.0, 320.0), 2.0);
        assert_eq!(safe_scale(0.0, 320.0), 1.0);
    }
}
