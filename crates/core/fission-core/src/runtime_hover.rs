//! Hover state refresh and clearing for the runtime.

use crate::input::hover::HoverController;
use crate::input::ControllerContext;
use crate::Runtime;
use anyhow::Result;
use fission_ir::CoreIR;
use fission_layout::{LayoutPoint, LayoutSize, LayoutSnapshot};

impl Runtime {
    /// Recomputes hover against a fresh layout at the last pointer position.
    pub(crate) fn refresh_hover_state(&mut self, ir: &CoreIR, layout: &LayoutSnapshot) -> bool {
        self.with_hover_controller(ir, layout, HoverController::refresh)
            .unwrap_or(false)
    }

    pub fn clear_hover_state(&mut self, ir: &CoreIR, point: Option<LayoutPoint>) -> Result<bool> {
        let layout = LayoutSnapshot::new(LayoutSize::ZERO);
        let input_time = self.clock().current_time();
        let (changed, actions) = {
            let mut ctx = self.hover_context(ir, &layout, input_time);
            let changed = HoverController::clear(&mut ctx, point);
            (changed, ctx.dispatched_actions)
        };
        self.dispatch_input_actions(actions)?;
        Ok(changed)
    }

    fn with_hover_controller(
        &mut self,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        f: impl FnOnce(&mut ControllerContext) -> bool,
    ) -> Option<bool> {
        let input_time = self.clock().current_time();
        let (changed, actions) = {
            let mut ctx = self.hover_context(ir, layout, input_time);
            let changed = f(&mut ctx);
            (changed, ctx.dispatched_actions)
        };
        self.dispatch_input_actions(actions).ok()?;
        Some(changed)
    }

    fn hover_context<'a>(
        &'a mut self,
        ir: &'a CoreIR,
        layout: &'a LayoutSnapshot,
        current_time: crate::time::CurrentTime,
    ) -> ControllerContext<'a> {
        ControllerContext {
            ir,
            layout,
            text_edit: &mut self.runtime_state.text_edit,
            selectable_text: &mut self.runtime_state.selectable_text,
            context_menu: &mut self.runtime_state.context_menu,
            interaction: &mut self.runtime_state.interaction,
            scroll: &mut self.runtime_state.scroll,
            viewport: &self.runtime_state.viewport,
            gesture: &mut self.runtime_state.gesture,
            editing_convention: self.editing_convention,
            current_time,
            clipboard: self.clipboard_backend.as_ref(),
            measurer: self.measurer.as_ref(),
            dispatched_actions: Vec::new(),
        }
    }
}
