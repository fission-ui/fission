use super::{ControllerContext, InputController};
use crate::event::{InputEvent, KeyCode, KeyEvent, PointerEvent, MOD_CTRL, MOD_SUPER};
use crate::ui::widgets::context_menu::{text_context_menu_button_id, TextContextMenuAction};
use fission_ir::{op::LayoutOp, Op, Semantics, WidgetId};
use fission_layout::{LayoutNodeGeometry, LayoutPoint};

pub struct SelectableTextController;

impl InputController for SelectableTextController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => self.handle_key(ctx, key_code.clone(), *modifiers),
            InputEvent::Pointer(PointerEvent::Down {
                point,
                button,
                modifiers,
                ..
            }) => {
                let hit =
                    crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point);
                if let (Some(owner), Some(hit_node_id)) = (ctx.context_menu.owner, hit) {
                    if let Some(action) = Self::toolbar_action_hit(ctx.ir, owner, hit_node_id) {
                        return self.execute_action(ctx, owner, action);
                    }
                }

                if !matches!(button, crate::event::PointerButton::Primary) {
                    return false;
                }

                let Some(hit_node_id) = hit else {
                    return false;
                };
                let Some((text_id, semantics)) = Self::selectable_text_at_hit(ctx, hit_node_id)
                else {
                    return false;
                };
                let caret = Self::caret_for_text(ctx, text_id, &semantics, *point);
                let state = ctx.selectable_text.get_mut_or_default(text_id);
                if !Self::has_shift(*modifiers) {
                    state.anchor = caret;
                }
                state.caret = caret;
                state.selecting = true;
                ctx.interaction.set_focused(Some(text_id));
                ctx.context_menu.close();
                true
            }
            InputEvent::Pointer(PointerEvent::Move { point, .. }) => {
                let Some(text_id) = Self::active_selection_owner(ctx) else {
                    return false;
                };
                let Some(semantics) = Self::selectable_text_semantics(ctx, text_id) else {
                    return false;
                };
                let caret = Self::caret_for_text(ctx, text_id, &semantics, *point);
                ctx.selectable_text.get_mut_or_default(text_id).caret = caret;
                true
            }
            InputEvent::Pointer(PointerEvent::Up { point, button, .. }) => {
                let Some(text_id) = Self::active_selection_owner(ctx) else {
                    return false;
                };
                let show_menu = ctx
                    .selectable_text
                    .get(text_id)
                    .and_then(|state| state.selection_range())
                    .is_some()
                    && matches!(button, crate::event::PointerButton::Secondary);
                ctx.selectable_text.get_mut_or_default(text_id).selecting = false;
                if show_menu {
                    ctx.context_menu.open(text_id, *point);
                }
                true
            }
            _ => false,
        }
    }
}

impl SelectableTextController {
    fn handle_key(
        &mut self,
        ctx: &mut ControllerContext,
        key_code: KeyCode,
        modifiers: u8,
    ) -> bool {
        let Some(owner) = ctx.interaction.focused else {
            return false;
        };
        let Some(semantics) = Self::selectable_text_semantics(ctx, owner) else {
            return false;
        };
        if !Self::has_primary_shortcut(modifiers) {
            return false;
        }
        match key_code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if let Some(selected) = Self::selected_text(ctx, owner, &semantics) {
                    if let Some(clipboard) = ctx.clipboard {
                        clipboard.set_text(&selected);
                    }
                }
                true
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let len = semantics.value.as_deref().unwrap_or("").len();
                let state = ctx.selectable_text.get_mut_or_default(owner);
                state.anchor = 0;
                state.caret = len;
                true
            }
            _ => false,
        }
    }

    fn selectable_text_at_hit(
        ctx: &ControllerContext,
        hit_node_id: WidgetId,
    ) -> Option<(WidgetId, Semantics)> {
        let mut current = Some(hit_node_id);
        while let Some(node_id) = current {
            let node = ctx.ir.nodes.get(&node_id)?;
            if let Op::Semantics(semantics) = &node.op {
                if semantics.selectable_text && !semantics.disabled {
                    return Some((node_id, semantics.clone()));
                }
            }
            current = node.parent;
        }
        None
    }

    fn selectable_text_semantics(ctx: &ControllerContext, owner: WidgetId) -> Option<Semantics> {
        let node = ctx.ir.nodes.get(&owner)?;
        match &node.op {
            Op::Semantics(semantics) if semantics.selectable_text => Some(semantics.clone()),
            _ => None,
        }
    }

    fn toolbar_action_hit(
        ir: &fission_ir::CoreIR,
        owner: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextContextMenuAction> {
        for action in [
            TextContextMenuAction::Copy,
            TextContextMenuAction::Cut,
            TextContextMenuAction::Paste,
            TextContextMenuAction::SelectAll,
        ] {
            if Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                text_context_menu_button_id(owner, action),
            ) {
                return Some(action);
            }
        }
        None
    }

    fn execute_action(
        &mut self,
        ctx: &mut ControllerContext,
        owner: WidgetId,
        action: TextContextMenuAction,
    ) -> bool {
        let Some(semantics) = Self::selectable_text_semantics(ctx, owner) else {
            return false;
        };
        match action {
            TextContextMenuAction::Copy => {
                if let Some(selected) = Self::selected_text(ctx, owner, &semantics) {
                    if let Some(clipboard) = ctx.clipboard {
                        clipboard.set_text(&selected);
                    }
                }
                ctx.context_menu.close();
                true
            }
            TextContextMenuAction::SelectAll => {
                let len = semantics.value.as_deref().unwrap_or("").len();
                let state = ctx.selectable_text.get_mut_or_default(owner);
                state.anchor = 0;
                state.caret = len;
                state.selecting = false;
                ctx.context_menu.close();
                true
            }
            TextContextMenuAction::Cut | TextContextMenuAction::Paste => true,
        }
    }

    fn selected_text(
        ctx: &ControllerContext,
        owner: WidgetId,
        semantics: &Semantics,
    ) -> Option<String> {
        let value = semantics.value.as_deref().unwrap_or("");
        let state = ctx.selectable_text.get(owner)?;
        let (anchor, caret) = state.selection_range()?;
        let start = Self::clamp_to_char_boundary(value, anchor.min(caret));
        let end = Self::clamp_to_char_boundary(value, anchor.max(caret));
        value.get(start..end).map(ToString::to_string)
    }

    fn caret_for_text(
        ctx: &ControllerContext,
        owner: WidgetId,
        semantics: &Semantics,
        point: LayoutPoint,
    ) -> usize {
        let value = semantics.value.as_deref().unwrap_or("");
        let Some((layout_id, geom)) = Self::layout_geometry(ctx, owner) else {
            return 0;
        };
        let local = Self::local_point(ctx, layout_id, geom, point);
        let Some(measurer) = ctx.measurer else {
            return 0;
        };
        let width = (geom.rect.size.width > 0.0).then_some(geom.rect.size.width);
        let caret = if let Some(runs) = Self::rich_runs(ctx.ir, owner) {
            measurer.hit_test_rich(&runs, width, local.x, local.y)
        } else {
            let font_size = Self::font_size(ctx.ir, owner).unwrap_or(14.0);
            measurer.hit_test(value, font_size, width, local.x, local.y)
        };
        Self::clamp_to_char_boundary(value, caret.min(value.len()))
    }

    fn layout_geometry<'a>(
        ctx: &'a ControllerContext,
        owner: WidgetId,
    ) -> Option<(WidgetId, &'a LayoutNodeGeometry)> {
        fn walk<'a>(
            ctx: &'a ControllerContext,
            node_id: WidgetId,
        ) -> Option<(WidgetId, &'a LayoutNodeGeometry)> {
            if let Some(geom) = ctx.layout.get_node_geometry(node_id) {
                return Some((node_id, geom));
            }
            for child in &ctx.ir.nodes.get(&node_id)?.children {
                if let Some(found) = walk(ctx, *child) {
                    return Some(found);
                }
            }
            None
        }
        walk(ctx, owner)
    }

    fn local_point(
        ctx: &ControllerContext,
        node_id: WidgetId,
        geom: &LayoutNodeGeometry,
        point: LayoutPoint,
    ) -> LayoutPoint {
        let mut scroll_x = 0.0;
        let mut scroll_y = 0.0;
        let mut walk = ctx.ir.nodes.get(&node_id).and_then(|node| node.parent);
        while let Some(parent_id) = walk {
            let Some(parent) = ctx.ir.nodes.get(&parent_id) else {
                break;
            };
            if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &parent.op {
                let offset = ctx.scroll.get_offset(parent_id);
                match direction {
                    fission_ir::FlexDirection::Row => scroll_x += offset,
                    fission_ir::FlexDirection::Column => scroll_y += offset,
                }
            }
            walk = parent.parent;
        }
        LayoutPoint::new(
            point.x - geom.rect.origin.x + scroll_x,
            point.y - geom.rect.origin.y + scroll_y,
        )
    }

    fn rich_runs(ir: &fission_ir::CoreIR, owner: WidgetId) -> Option<Vec<fission_ir::op::TextRun>> {
        fn walk(
            ir: &fission_ir::CoreIR,
            node_id: WidgetId,
        ) -> Option<Vec<fission_ir::op::TextRun>> {
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) if !runs.is_empty() => {
                    Some(runs.clone())
                }
                _ => node.children.iter().find_map(|child| walk(ir, *child)),
            }
        }
        walk(ir, owner)
    }

    fn font_size(ir: &fission_ir::CoreIR, owner: WidgetId) -> Option<f32> {
        fn walk(ir: &fission_ir::CoreIR, node_id: WidgetId) -> Option<f32> {
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawText { size, .. }) => Some(*size),
                Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    runs.first().map(|run| run.style.font_size)
                }
                _ => node.children.iter().find_map(|child| walk(ir, *child)),
            }
        }
        walk(ir, owner)
    }

    fn active_selection_owner(ctx: &ControllerContext) -> Option<WidgetId> {
        ctx.selectable_text
            .states
            .iter()
            .find_map(|(id, state)| state.selecting.then_some(*id))
    }

    fn node_or_ancestor_matches(
        ir: &fission_ir::CoreIR,
        node_id: WidgetId,
        expected: WidgetId,
    ) -> bool {
        let mut current = Some(node_id);
        while let Some(id) = current {
            if id == expected {
                return true;
            }
            current = ir.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    fn clamp_to_char_boundary(value: &str, mut index: usize) -> usize {
        index = index.min(value.len());
        while index > 0 && !value.is_char_boundary(index) {
            index -= 1;
        }
        index
    }

    fn has_shift(modifiers: u8) -> bool {
        (modifiers & crate::event::MOD_SHIFT) != 0
    }

    fn has_primary_shortcut(modifiers: u8) -> bool {
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            (modifiers & MOD_SUPER) != 0
        } else {
            (modifiers & MOD_CTRL) != 0
        }
    }
}
