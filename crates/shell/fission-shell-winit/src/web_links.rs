use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use fission_core::LinkTarget;
use fission_ir::WidgetId;
use fission_render::LayoutSize;
use fission_test_driver::TestEvent;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{HtmlCanvasElement, HtmlElement, MouseEvent};
use winit::event_loop::EventLoopProxy;

use crate::SemanticRecord;

/// Projects canvas semantics into genuine, transparent browser anchors.
///
/// The visual authority remains Fission's renderer. The DOM projection gives
/// browsers link inspection, context menus, keyboard focus, accessibility, and
/// standards-based href/target/download/popover behavior.
pub(crate) struct WebLinkOverlay {
    canvas: HtmlCanvasElement,
    root: HtmlElement,
    click_handlers: Vec<Closure<dyn FnMut(MouseEvent)>>,
}

impl WebLinkOverlay {
    pub(crate) fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let document = canvas
            .owner_document()
            .ok_or_else(|| "web canvas has no owner document".to_string())?;
        let root = document
            .create_element("div")
            .map_err(crate::js_error_to_string)?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "browser created a non-HTML link overlay".to_string())?;
        root.set_attribute("data-fission-link-overlay", "")
            .map_err(crate::js_error_to_string)?;
        root.set_attribute(
            "style",
            "position:fixed;inset:0;overflow:hidden;pointer-events:none;z-index:2147483646;",
        )
        .map_err(crate::js_error_to_string)?;
        document
            .body()
            .ok_or_else(|| "browser document has no body".to_string())?
            .append_child(&root)
            .map_err(crate::js_error_to_string)?;
        Ok(Self {
            canvas,
            root,
            click_handlers: Vec::new(),
        })
    }

    pub(crate) fn sync(
        &mut self,
        records: &[SemanticRecord],
        viewport: LayoutSize,
        activations: &Rc<RefCell<VecDeque<WidgetId>>>,
        proxy: &EventLoopProxy<TestEvent>,
    ) -> Result<(), String> {
        self.root.set_text_content(None);
        self.click_handlers.clear();
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return Ok(());
        }

        let document = self
            .root
            .owner_document()
            .ok_or_else(|| "link overlay has no owner document".to_string())?;
        let canvas_rect = self.canvas.get_bounding_client_rect();
        let scale_x = canvas_rect.width() / f64::from(viewport.width);
        let scale_y = canvas_rect.height() / f64::from(viewport.height);

        for record in records {
            let Some(link) = &record.semantics.hyperlink else {
                continue;
            };
            let Some(bounds) = record.node.visible_bounds else {
                continue;
            };
            if bounds.width <= 0.0 || bounds.height <= 0.0 {
                continue;
            }
            let anchor = document
                .create_element("a")
                .map_err(crate::js_error_to_string)?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "browser created a non-HTML anchor".to_string())?;
            anchor
                .set_attribute("href", &link.href)
                .map_err(crate::js_error_to_string)?;
            anchor
                .set_attribute("target", link.target.as_html_target())
                .map_err(crate::js_error_to_string)?;
            if let Some(rel) = &link.rel {
                anchor
                    .set_attribute("rel", rel)
                    .map_err(crate::js_error_to_string)?;
            } else if matches!(link.target, LinkTarget::NewWindow) {
                anchor
                    .set_attribute("rel", "noopener noreferrer")
                    .map_err(crate::js_error_to_string)?;
            }
            if let Some(download) = &link.download {
                anchor
                    .set_attribute("download", download)
                    .map_err(crate::js_error_to_string)?;
            }
            if let Some(popover) = &record.semantics.popover_target {
                anchor
                    .set_attribute("popovertarget", &popover.id)
                    .map_err(crate::js_error_to_string)?;
                anchor
                    .set_attribute("popovertargetaction", popover.action.as_html_action())
                    .map_err(crate::js_error_to_string)?;
            }
            if let Some(label) = &record.semantics.label {
                anchor
                    .set_attribute("aria-label", label)
                    .map_err(crate::js_error_to_string)?;
            }
            anchor
                .set_attribute("data-fission-widget-id", &record.id.to_string())
                .map_err(crate::js_error_to_string)?;
            anchor
                .set_attribute(
                    "style",
                    &format!(
                        "position:absolute;left:{:.3}px;top:{:.3}px;width:{:.3}px;height:{:.3}px;display:block;pointer-events:auto;color:transparent;background:transparent;outline-offset:2px;",
                        canvas_rect.left() + f64::from(bounds.x) * scale_x,
                        canvas_rect.top() + f64::from(bounds.y) * scale_y,
                        f64::from(bounds.width) * scale_x,
                        f64::from(bounds.height) * scale_y,
                    ),
                )
                .map_err(crate::js_error_to_string)?;

            // Let the browser own modified clicks, downloads, and links that
            // intentionally target another browsing context. A normal
            // same-context click enters Fission's action/effect pipeline so
            // custom reducers and SPA history observe one activation.
            let intercept = matches!(link.target, LinkTarget::Current)
                && link.download.is_none()
                && record.semantics.popover_target.is_none();
            let target = record.id;
            let activations = activations.clone();
            let proxy = proxy.clone();
            let handler = Closure::wrap(Box::new(move |event: MouseEvent| {
                if !intercept
                    || event.meta_key()
                    || event.ctrl_key()
                    || event.shift_key()
                    || event.alt_key()
                {
                    return;
                }
                event.prevent_default();
                activations.borrow_mut().push_back(target);
                let _ = proxy.send_event(TestEvent::Wake);
            }) as Box<dyn FnMut(_)>);
            anchor
                .add_event_listener_with_callback("click", handler.as_ref().unchecked_ref())
                .map_err(crate::js_error_to_string)?;
            self.root
                .append_child(&anchor)
                .map_err(crate::js_error_to_string)?;
            self.click_handlers.push(handler);
        }
        Ok(())
    }
}

impl Drop for WebLinkOverlay {
    fn drop(&mut self) {
        if let Some(parent) = self.root.parent_node() {
            let _ = parent.remove_child(&self.root);
        }
    }
}
