use super::{ControllerContext, InputController};
use crate::env::{SelectionGranularity, TextSelectionHandleKind};
use crate::event::{EditingCommand, GestureEvent, InputEvent, KeyCode, KeyEvent, PointerEvent};
use crate::selection::{
    clear_other_regions, document_for_selection_owner, set_region_selection, RegionDocument,
    SelectionRegionCommand, TextRegionPosition, TextRegionSelection,
};
use crate::ui::widgets::context_menu::{text_context_menu_button_id, TextContextMenuAction};
use crate::ui::widgets::selection_region::{
    region_runtime_config, selection_region_handle_id, SelectionRegionControls,
};
use crate::{TextAffinity, TextPosition};
use fission_ir::{op::LayoutOp, Op, Semantics, WidgetId};
use fission_layout::{LayoutNodeGeometry, LayoutPoint};
use unicode_segmentation::UnicodeSegmentation;

const MULTI_CLICK_INTERVAL_MS: u64 = 500;
const MULTI_CLICK_SLOP: f32 = 6.0;
const LONG_PRESS_INTERVAL_MS: u64 = 500;

pub struct SelectableTextController;

#[derive(Clone)]
struct SelectionTarget {
    region_id: WidgetId,
    member_id: WidgetId,
    semantics: Semantics,
}

impl InputController for SelectableTextController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => self.handle_key(ctx, key_code.clone(), *modifiers),
            InputEvent::Editing(command) => self.handle_editing_command(ctx, command),
            InputEvent::Gesture(GestureEvent::DoubleTap { point }) => {
                self.select_gesture(ctx, *point, SelectionGranularity::Word, false)
            }
            InputEvent::Gesture(GestureEvent::LongPress { point }) => {
                self.select_gesture(ctx, *point, SelectionGranularity::Word, true)
            }
            InputEvent::Pointer(PointerEvent::Down {
                point,
                button,
                modifiers,
                kind,
                ..
            }) => self.pointer_down(ctx, *point, button, *modifiers, *kind),
            InputEvent::Pointer(PointerEvent::Move { point, .. }) => self.pointer_move(ctx, *point),
            InputEvent::Pointer(PointerEvent::Up {
                point,
                button,
                kind,
                ..
            }) => self.pointer_up(ctx, *point, button, *kind),
            InputEvent::Pointer(PointerEvent::Cancel { .. }) => {
                let Some(region_id) = Self::active_region(ctx) else {
                    return false;
                };
                ctx.selectable_text
                    .region_mut_or_default(region_id)
                    .selecting = false;
                let state = ctx.selectable_text.region_mut_or_default(region_id);
                state.drag_started = false;
                state.active_handle = None;
                state.magnifier_visible = false;
                true
            }
            _ => false,
        }
    }
}

impl SelectableTextController {
    fn pointer_down(
        &mut self,
        ctx: &mut ControllerContext,
        point: LayoutPoint,
        button: &crate::event::PointerButton,
        modifiers: u8,
        kind: crate::event::PointerKind,
    ) -> bool {
        let hit = crate::hit_test::hit_test_with_viewports(
            ctx.ir,
            ctx.layout,
            ctx.scroll,
            ctx.viewport,
            point,
        );
        if let (Some(owner), Some(hit_node_id)) = (ctx.context_menu.owner, hit) {
            if let Some(action) = Self::toolbar_action_hit(ctx.ir, owner, hit_node_id) {
                return self.execute_action(ctx, owner, action);
            }
        }
        if let (Some(owner), Some(hit_node_id)) = (ctx.interaction.focused, hit) {
            if let Some(handle) = Self::selection_handle_hit(ctx.ir, owner, hit_node_id) {
                if matches!(button, crate::event::PointerButton::Primary) {
                    let magnifier_visible = Self::uses_touch_affordances(ctx, owner);
                    let state = ctx.selectable_text.region_mut_or_default(owner);
                    state.active_handle = Some(handle);
                    state.selecting = true;
                    state.drag_started = true;
                    state.pointer_kind = kind;
                    state.pointer_down_at = Some(ctx.current_time);
                    state.pointer_down_point = Some(point);
                    state.magnifier_visible = magnifier_visible;
                    ctx.context_menu.close();
                    return true;
                }
            }
        }
        if matches!(button, crate::event::PointerButton::Secondary) {
            let Some(target) = Self::target_at_point(ctx, point) else {
                return false;
            };
            let Some(document) = document_for_selection_owner(ctx.ir, target.region_id) else {
                return false;
            };
            let existing = Self::selection_for_owner(ctx, target.region_id, &document);
            let caret = Self::position_for_target(ctx, &target, point);
            let caret_offset = document.position_offset(caret).unwrap_or(0);
            let select_clicked_word = existing.is_none_or(|selection| {
                let base = document.position_offset(selection.base).unwrap_or(0);
                let extent = document.position_offset(selection.extent).unwrap_or(0);
                selection.is_collapsed()
                    || caret_offset < base.min(extent)
                    || caret_offset > base.max(extent)
            });
            if select_clicked_word {
                let selection =
                    Self::granular_selection(&document, caret, SelectionGranularity::Word);
                if set_region_selection(ctx.selectable_text, target.region_id, &document, selection)
                    .is_err()
                {
                    return false;
                }
            }
            ctx.interaction.set_focused(Some(target.region_id));
            ctx.selectable_text
                .region_mut_or_default(target.region_id)
                .pointer_kind = kind;
            ctx.context_menu.open(target.region_id, point);
            Self::sync_affordances(ctx, target.region_id, &document, false);
            return true;
        }
        if !matches!(button, crate::event::PointerButton::Primary) {
            return false;
        }
        let Some(target) = Self::target_at_point(ctx, point) else {
            return false;
        };
        let Some(document) = document_for_selection_owner(ctx.ir, target.region_id) else {
            return false;
        };
        let caret = Self::position_for_target(ctx, &target, point);
        clear_other_regions(ctx.selectable_text, ctx.ir, target.region_id);
        let controls = Self::controls(ctx, target.region_id);
        let click_count = if kind == crate::event::PointerKind::Mouse {
            Self::next_click_count(ctx, target.region_id, point)
        } else {
            1
        };
        let (selection, granularity) = if Self::has_shift(modifiers) {
            let base = Self::selection_for_owner(ctx, target.region_id, &document)
                .map(|selection| selection.base)
                .unwrap_or(caret);
            (
                TextRegionSelection {
                    base,
                    extent: caret,
                    affinity: TextAffinity::Downstream,
                },
                SelectionGranularity::Character,
            )
        } else if click_count == 2 && controls.word_selection_on_double_click {
            (
                Self::granular_selection(&document, caret, SelectionGranularity::Word),
                SelectionGranularity::Word,
            )
        } else if click_count >= 3 && controls.paragraph_selection_on_triple_click {
            (
                Self::granular_selection(&document, caret, SelectionGranularity::Paragraph),
                SelectionGranularity::Paragraph,
            )
        } else {
            (
                TextRegionSelection::collapsed(caret),
                SelectionGranularity::Character,
            )
        };
        if set_region_selection(ctx.selectable_text, target.region_id, &document, selection)
            .is_err()
        {
            return false;
        }
        let state = ctx.selectable_text.region_mut_or_default(target.region_id);
        state.selecting = true;
        state.granularity = granularity;
        state.pointer_down_at = Some(ctx.current_time);
        state.pointer_down_point = Some(point);
        state.pointer_kind = kind;
        state.drag_started = kind == crate::event::PointerKind::Mouse;
        ctx.interaction.set_focused(Some(target.region_id));
        ctx.context_menu.close();
        Self::sync_affordances(ctx, target.region_id, &document, false);
        true
    }

    fn pointer_move(&mut self, ctx: &mut ControllerContext, point: LayoutPoint) -> bool {
        let Some(region_id) = Self::active_region(ctx) else {
            return false;
        };
        let Some(document) = document_for_selection_owner(ctx.ir, region_id) else {
            return false;
        };
        let controls = Self::controls(ctx, region_id);
        let (pointer_kind, down_point, drag_started, active_handle) = ctx
            .selectable_text
            .region(region_id)
            .map(|state| {
                (
                    state.pointer_kind,
                    state.pointer_down_point,
                    state.drag_started,
                    state.active_handle,
                )
            })
            .unwrap_or_default();
        if matches!(
            pointer_kind,
            crate::event::PointerKind::Touch | crate::event::PointerKind::Stylus
        ) && !drag_started
        {
            if down_point
                .is_some_and(|origin| Self::distance(origin, point) < controls.touch_slop.max(0.0))
            {
                return true;
            }
            ctx.selectable_text
                .region_mut_or_default(region_id)
                .drag_started = true;
        }
        if controls.edge_auto_scroll {
            Self::edge_auto_scroll(ctx, region_id, point, &controls);
        }
        let Some(target) = Self::target_for_active_region(ctx, region_id, point, &document) else {
            return false;
        };
        let caret = Self::position_for_target(ctx, &target, point);
        let Some(current) = ctx.selectable_text.region_selection(region_id) else {
            return false;
        };
        let granularity = ctx
            .selectable_text
            .region(region_id)
            .map_or(SelectionGranularity::Character, |state| state.granularity);
        let selection = match active_handle {
            Some(TextSelectionHandleKind::Caret) => TextRegionSelection::collapsed(caret),
            Some(TextSelectionHandleKind::Start) => {
                if document.position_offset(current.base).unwrap_or(0)
                    <= document.position_offset(current.extent).unwrap_or(0)
                {
                    TextRegionSelection {
                        base: caret,
                        ..current
                    }
                } else {
                    TextRegionSelection {
                        extent: caret,
                        ..current
                    }
                }
            }
            Some(TextSelectionHandleKind::End) => {
                if document.position_offset(current.base).unwrap_or(0)
                    >= document.position_offset(current.extent).unwrap_or(0)
                {
                    TextRegionSelection {
                        base: caret,
                        ..current
                    }
                } else {
                    TextRegionSelection {
                        extent: caret,
                        ..current
                    }
                }
            }
            None => TextRegionSelection {
                base: current.base,
                extent: Self::extent_for_drag(&document, current.base, caret, granularity),
                affinity: current.affinity,
            },
        };
        if set_region_selection(ctx.selectable_text, region_id, &document, selection).is_err() {
            return false;
        }
        let touch_affordances = controls.platform_style.uses_touch_affordances(pointer_kind);
        let state = ctx.selectable_text.region_mut_or_default(region_id);
        state.selecting = true;
        state.magnifier_visible = touch_affordances;
        Self::sync_affordances(ctx, region_id, &document, touch_affordances);
        true
    }

    fn pointer_up(
        &mut self,
        ctx: &mut ControllerContext,
        point: LayoutPoint,
        button: &crate::event::PointerButton,
        kind: crate::event::PointerKind,
    ) -> bool {
        if !matches!(button, crate::event::PointerButton::Primary) {
            return false;
        }
        let Some(region_id) = Self::active_region(ctx) else {
            return false;
        };
        let controls = Self::controls(ctx, region_id);
        let (down_at, down_point, drag_started) = ctx
            .selectable_text
            .region(region_id)
            .map(|state| {
                (
                    state.pointer_down_at,
                    state.pointer_down_point,
                    state.drag_started,
                )
            })
            .unwrap_or_default();
        let held = matches!(
            kind,
            crate::event::PointerKind::Touch | crate::event::PointerKind::Stylus
        ) && !drag_started
            && down_at.is_some_and(|started| {
                ctx.current_time.saturating_sub(started) >= LONG_PRESS_INTERVAL_MS
            })
            && down_point.is_some_and(|origin| {
                Self::distance(origin, point) <= controls.touch_slop.max(0.0)
            });
        if held && controls.word_selection_on_long_press {
            let _ = self.select_gesture(ctx, point, SelectionGranularity::Word, true);
        }
        let document = document_for_selection_owner(ctx.ir, region_id);
        let show_mobile_toolbar = controls.platform_style.uses_touch_affordances(kind)
            && document.as_ref().is_some_and(|document| {
                Self::selection_for_owner(ctx, region_id, document)
                    .is_some_and(|selection| !selection.is_collapsed())
            });
        let state = ctx.selectable_text.region_mut_or_default(region_id);
        state.selecting = false;
        state.drag_started = false;
        state.active_handle = None;
        state.magnifier_visible = false;
        if show_mobile_toolbar {
            ctx.context_menu.open(region_id, point);
        }
        if let Some(document) = document.as_ref() {
            Self::sync_affordances(ctx, region_id, document, false);
        }
        true
    }

    fn select_gesture(
        &mut self,
        ctx: &mut ControllerContext,
        point: LayoutPoint,
        granularity: SelectionGranularity,
        show_menu: bool,
    ) -> bool {
        let Some(target) = Self::target_at_point(ctx, point) else {
            return false;
        };
        let controls = Self::controls(ctx, target.region_id);
        if (!show_menu && !controls.word_selection_on_double_click)
            || (show_menu && !controls.word_selection_on_long_press)
        {
            return false;
        }
        let Some(document) = document_for_selection_owner(ctx.ir, target.region_id) else {
            return false;
        };
        let caret = Self::position_for_target(ctx, &target, point);
        let selection = Self::granular_selection(&document, caret, granularity);
        if set_region_selection(ctx.selectable_text, target.region_id, &document, selection)
            .is_err()
        {
            return false;
        }
        let state = ctx.selectable_text.region_mut_or_default(target.region_id);
        state.selecting = false;
        state.granularity = granularity;
        state.pointer_kind = crate::event::PointerKind::Touch;
        state.drag_started = false;
        state.active_handle = None;
        state.magnifier_visible = false;
        ctx.interaction.set_focused(Some(target.region_id));
        if show_menu && !selection.is_collapsed() && controls.context_menu.enabled {
            ctx.context_menu.open(target.region_id, point);
        }
        Self::sync_affordances(ctx, target.region_id, &document, false);
        true
    }

    fn handle_editing_command(
        &mut self,
        ctx: &mut ControllerContext,
        command: &EditingCommand,
    ) -> bool {
        let Some(owner) = ctx.interaction.focused else {
            return false;
        };
        let Some(document) = document_for_selection_owner(ctx.ir, owner) else {
            return false;
        };
        match command {
            EditingCommand::Copy => {
                if let Some(selection) = Self::selection_for_owner(ctx, owner, &document) {
                    if let Some(selected) = document.selected_text(selection) {
                        if let Some(clipboard) = ctx.clipboard {
                            clipboard.set_text(&selected);
                        }
                    }
                }
                true
            }
            EditingCommand::SelectAll => {
                let handled = crate::selection::apply_region_command(
                    ctx.selectable_text,
                    ctx.ir,
                    owner,
                    SelectionRegionCommand::SelectAll,
                )
                .is_ok();
                if handled {
                    Self::sync_affordances(ctx, owner, &document, false);
                }
                handled
            }
            EditingCommand::Cut
            | EditingCommand::Paste(_)
            | EditingCommand::Undo
            | EditingCommand::Redo => false,
        }
    }

    fn handle_key(
        &mut self,
        ctx: &mut ControllerContext,
        key_code: KeyCode,
        modifiers: u8,
    ) -> bool {
        let primary = ctx.editing_convention.has_primary_shortcut(modifiers)
            && !ctx.editing_convention.is_alt_gr(modifiers);
        if primary {
            let command = match key_code {
                KeyCode::Char('c') | KeyCode::Char('C') => Some(EditingCommand::Copy),
                KeyCode::Char('a') | KeyCode::Char('A') => Some(EditingCommand::SelectAll),
                _ => None,
            };
            if let Some(command) = command {
                return self.handle_editing_command(ctx, &command);
            }
        }
        match key_code {
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End => {
                self.handle_navigation(ctx, key_code, Self::has_shift(modifiers), primary)
            }
            _ => false,
        }
    }

    fn handle_navigation(
        &mut self,
        ctx: &mut ControllerContext,
        key_code: KeyCode,
        extend: bool,
        document_boundary: bool,
    ) -> bool {
        let Some(region_id) = ctx.interaction.focused else {
            return false;
        };
        let Some(document) = document_for_selection_owner(ctx.ir, region_id) else {
            return false;
        };
        let Some(current) = Self::selection_for_owner(ctx, region_id, &document) else {
            return false;
        };
        let target = if !extend && !current.is_collapsed() {
            let base = document.position_offset(current.base).unwrap_or(0);
            let extent = document.position_offset(current.extent).unwrap_or(0);
            match key_code {
                KeyCode::Left | KeyCode::Up | KeyCode::Home => {
                    if base <= extent {
                        current.base
                    } else {
                        current.extent
                    }
                }
                _ => {
                    if base >= extent {
                        current.base
                    } else {
                        current.extent
                    }
                }
            }
        } else {
            Self::move_position(&document, current.extent, key_code, document_boundary)
        };
        let selection = if extend {
            TextRegionSelection {
                extent: target,
                ..current
            }
        } else {
            TextRegionSelection::collapsed(target)
        };
        if set_region_selection(ctx.selectable_text, region_id, &document, selection).is_err() {
            return false;
        }
        ctx.selectable_text
            .region_mut_or_default(region_id)
            .granularity = SelectionGranularity::Character;
        Self::sync_affordances(ctx, region_id, &document, false);
        true
    }

    fn move_position(
        document: &RegionDocument,
        at: TextRegionPosition,
        key_code: KeyCode,
        document_boundary: bool,
    ) -> TextRegionPosition {
        let Some(index) = document
            .members
            .iter()
            .position(|member| member.node_id == at.node_id)
        else {
            return at;
        };
        let member = &document.members[index];
        let offset = at.offset.utf8_offset().min(member.text.len());
        match key_code {
            KeyCode::Left => {
                if offset == 0 {
                    return index
                        .checked_sub(1)
                        .map(|previous| {
                            let previous = &document.members[previous];
                            TextRegionPosition::at(
                                previous.node_id,
                                TextPosition::at_end(&previous.text),
                            )
                        })
                        .unwrap_or(at);
                }
                let previous = member.text[..offset]
                    .grapheme_indices(true)
                    .next_back()
                    .map_or(0, |(start, _)| start);
                TextRegionPosition::at(member.node_id, TextPosition::floor(&member.text, previous))
            }
            KeyCode::Right => {
                if offset == member.text.len() {
                    return document
                        .members
                        .get(index + 1)
                        .map(|next| TextRegionPosition::at(next.node_id, TextPosition::START))
                        .unwrap_or(at);
                }
                let next = member.text[offset..]
                    .graphemes(true)
                    .next()
                    .map_or(member.text.len(), |grapheme| offset + grapheme.len());
                TextRegionPosition::at(member.node_id, TextPosition::floor(&member.text, next))
            }
            KeyCode::Up => index
                .checked_sub(1)
                .and_then(|previous| document.members.get(previous))
                .map(|previous| {
                    TextRegionPosition::at(
                        previous.node_id,
                        TextPosition::floor(&previous.text, offset.min(previous.text.len())),
                    )
                })
                .unwrap_or(at),
            KeyCode::Down => document
                .members
                .get(index + 1)
                .map(|next| {
                    TextRegionPosition::at(
                        next.node_id,
                        TextPosition::floor(&next.text, offset.min(next.text.len())),
                    )
                })
                .unwrap_or(at),
            KeyCode::Home if document_boundary => document
                .members
                .first()
                .map(|first| TextRegionPosition::at(first.node_id, TextPosition::START))
                .unwrap_or(at),
            KeyCode::End if document_boundary => document
                .members
                .last()
                .map(|last| TextRegionPosition::at(last.node_id, TextPosition::at_end(&last.text)))
                .unwrap_or(at),
            KeyCode::Home => TextRegionPosition::at(member.node_id, TextPosition::START),
            KeyCode::End => {
                TextRegionPosition::at(member.node_id, TextPosition::at_end(&member.text))
            }
            _ => at,
        }
    }

    fn controls(ctx: &ControllerContext, region_id: WidgetId) -> SelectionRegionControls {
        region_runtime_config(ctx.ir, region_id)
            .map(|config| config.controls.clone())
            .unwrap_or_default()
    }

    fn uses_touch_affordances(ctx: &ControllerContext, region_id: WidgetId) -> bool {
        let controls = Self::controls(ctx, region_id);
        let pointer = ctx
            .selectable_text
            .region(region_id)
            .map_or(crate::event::PointerKind::default(), |state| {
                state.pointer_kind
            });
        controls.platform_style.uses_touch_affordances(pointer)
    }

    fn sync_affordances(
        ctx: &mut ControllerContext,
        region_id: WidgetId,
        document: &RegionDocument,
        magnifier_visible: bool,
    ) {
        let Some(selection) = Self::selection_for_owner(ctx, region_id, document) else {
            return;
        };
        let base_point = Self::caret_point(ctx, region_id, selection.base);
        let extent_point = Self::caret_point(ctx, region_id, selection.extent);
        let base_offset = document.position_offset(selection.base).unwrap_or(0);
        let extent_offset = document.position_offset(selection.extent).unwrap_or(0);
        let state = ctx.selectable_text.region_mut_or_default(region_id);
        state.magnifier_visible = magnifier_visible;
        state.magnifier_anchor = magnifier_visible.then_some(extent_point).flatten();
        if selection.is_collapsed() {
            state.caret_handle = extent_point;
            state.selection_start_handle = None;
            state.selection_end_handle = None;
        } else {
            state.caret_handle = None;
            if base_offset <= extent_offset {
                state.selection_start_handle = base_point;
                state.selection_end_handle = extent_point;
            } else {
                state.selection_start_handle = extent_point;
                state.selection_end_handle = base_point;
            }
        }
    }

    fn caret_point(
        ctx: &ControllerContext,
        region_id: WidgetId,
        position: TextRegionPosition,
    ) -> Option<LayoutPoint> {
        let region_geom = ctx
            .layout
            .get_node_geometry(region_id)
            .or_else(|| Self::layout_geometry(ctx, region_id).map(|(_, geometry)| geometry))?;
        let (layout_id, member_geom) = Self::layout_geometry(ctx, position.node_id)?;
        let semantics = crate::selection::selectable_semantics(ctx.ir, position.node_id)?;
        let text = semantics.value.as_deref().unwrap_or_default();
        let offset = position.offset.utf8_offset().min(text.len());
        let paint_id = Self::text_paint_node(ctx.ir, position.node_id).unwrap_or(layout_id);
        let paint_geom = ctx
            .layout
            .get_node_geometry(paint_id)
            .unwrap_or(member_geom);
        let (x, y, height) = if let Some(paragraph) = ctx.layout.get_resolved_paragraph(paint_id) {
            let caret = paragraph.caret(offset, false)?;
            (caret.position.x, caret.position.y, caret.height.max(1.0))
        } else {
            let measurer = ctx.measurer?;
            let font_size = Self::font_size(ctx.ir, position.node_id).unwrap_or(14.0);
            let width = (paint_geom.rect.size.width > 0.0).then_some(paint_geom.rect.size.width);
            let (x, y) = measurer.get_caret_position(text, font_size, width, offset);
            let height = measurer
                .get_line_metrics(text, font_size, width)
                .into_iter()
                .find(|line| offset >= line.start_index && offset <= line.end_index)
                .map_or(font_size * 1.25, |line| line.height)
                .max(1.0);
            (x, y, height)
        };
        Some(LayoutPoint::new(
            paint_geom.rect.origin.x - region_geom.rect.origin.x + x,
            paint_geom.rect.origin.y - region_geom.rect.origin.y + y + height,
        ))
    }

    fn text_paint_node(ir: &fission_ir::CoreIR, root: WidgetId) -> Option<WidgetId> {
        let node = ir.nodes.get(&root)?;
        if matches!(
            &node.op,
            Op::Paint(
                fission_ir::PaintOp::DrawText { .. } | fission_ir::PaintOp::DrawRichText { .. }
            )
        ) {
            return Some(root);
        }
        node.children
            .iter()
            .find_map(|child| Self::text_paint_node(ir, *child))
    }

    fn edge_auto_scroll(
        ctx: &mut ControllerContext,
        region_id: WidgetId,
        point: LayoutPoint,
        controls: &SelectionRegionControls,
    ) {
        let mut current = ctx.ir.nodes.get(&region_id).and_then(|node| node.parent);
        while let Some(node_id) = current {
            let Some(node) = ctx.ir.nodes.get(&node_id) else {
                break;
            };
            if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
                if let Some(geometry) = ctx.layout.get_node_geometry(node_id) {
                    let threshold = controls.edge_auto_scroll_threshold.max(1.0).min(
                        if *direction == fission_ir::FlexDirection::Row {
                            geometry.rect.size.width * 0.5
                        } else {
                            geometry.rect.size.height * 0.5
                        },
                    );
                    let delta = match direction {
                        fission_ir::FlexDirection::Row
                            if point.x < geometry.rect.origin.x + threshold =>
                        {
                            -controls.edge_auto_scroll_step
                        }
                        fission_ir::FlexDirection::Row
                            if point.x > geometry.rect.right() - threshold =>
                        {
                            controls.edge_auto_scroll_step
                        }
                        fission_ir::FlexDirection::Column
                            if point.y < geometry.rect.origin.y + threshold =>
                        {
                            -controls.edge_auto_scroll_step
                        }
                        fission_ir::FlexDirection::Column
                            if point.y > geometry.rect.bottom() - threshold =>
                        {
                            controls.edge_auto_scroll_step
                        }
                        _ => 0.0,
                    };
                    if delta != 0.0 {
                        let viewport = if *direction == fission_ir::FlexDirection::Row {
                            geometry.rect.size.width
                        } else {
                            geometry.rect.size.height
                        };
                        let content = if *direction == fission_ir::FlexDirection::Row {
                            geometry.content_size.width
                        } else {
                            geometry.content_size.height
                        };
                        let max_offset = (content - viewport).max(0.0);
                        let offset =
                            (ctx.scroll.get_offset(node_id) + delta).clamp(0.0, max_offset);
                        ctx.scroll.set_offset(node_id, offset);
                    }
                }
                return;
            }
            current = node.parent;
        }
    }

    fn selection_for_owner(
        ctx: &ControllerContext,
        owner: WidgetId,
        document: &RegionDocument,
    ) -> Option<TextRegionSelection> {
        ctx.selectable_text.region_selection(owner).or_else(|| {
            let member = document.members.first()?;
            if member.node_id != owner || document.members.len() != 1 {
                return None;
            }
            let state = ctx.selectable_text.get(owner)?;
            Some(TextRegionSelection {
                base: TextRegionPosition::at(
                    owner,
                    TextPosition::floor(&member.text, state.anchor),
                ),
                extent: TextRegionPosition::at(
                    owner,
                    TextPosition::floor(&member.text, state.caret),
                ),
                affinity: TextAffinity::Downstream,
            })
        })
    }

    fn next_click_count(
        ctx: &mut ControllerContext,
        region_id: WidgetId,
        point: LayoutPoint,
    ) -> u8 {
        let state = ctx.selectable_text.region_mut_or_default(region_id);
        let repeated = state
            .last_click_at
            .is_some_and(|last| ctx.current_time.saturating_sub(last) <= MULTI_CLICK_INTERVAL_MS)
            && state
                .last_click_point
                .is_some_and(|last| Self::distance(last, point) <= MULTI_CLICK_SLOP);
        state.click_count = if repeated {
            (state.click_count % 3) + 1
        } else {
            1
        };
        state.last_click_at = Some(ctx.current_time);
        state.last_click_point = Some(point);
        state.click_count
    }

    fn position_for_target(
        ctx: &ControllerContext,
        target: &SelectionTarget,
        point: LayoutPoint,
    ) -> TextRegionPosition {
        let text = target.semantics.value.as_deref().unwrap_or("");
        let caret = Self::caret_for_text(ctx, target.member_id, &target.semantics, point);
        TextRegionPosition::at(target.member_id, TextPosition::floor(text, caret))
    }

    fn granular_selection(
        document: &RegionDocument,
        at: TextRegionPosition,
        granularity: SelectionGranularity,
    ) -> TextRegionSelection {
        let Some(member) = document
            .members
            .iter()
            .find(|member| member.node_id == at.node_id)
        else {
            return TextRegionSelection::collapsed(at);
        };
        let offset = at.offset.utf8_offset();
        let (start, end) = match granularity {
            SelectionGranularity::Character => (offset, offset),
            SelectionGranularity::Word => Self::word_range(&member.text, offset),
            SelectionGranularity::Paragraph => Self::paragraph_range(&member.text, offset),
        };
        TextRegionSelection {
            base: TextRegionPosition::at(member.node_id, TextPosition::floor(&member.text, start)),
            extent: TextRegionPosition::at(member.node_id, TextPosition::floor(&member.text, end)),
            affinity: TextAffinity::Downstream,
        }
    }

    fn extent_for_drag(
        document: &RegionDocument,
        base: TextRegionPosition,
        at: TextRegionPosition,
        granularity: SelectionGranularity,
    ) -> TextRegionPosition {
        if granularity == SelectionGranularity::Character {
            return at;
        }
        let selection = Self::granular_selection(document, at, granularity);
        if document.position_offset(at).unwrap_or(0) < document.position_offset(base).unwrap_or(0) {
            selection.base
        } else {
            selection.extent
        }
    }

    fn word_range(text: &str, offset: usize) -> (usize, usize) {
        if text.is_empty() {
            return (0, 0);
        }
        let offset = TextPosition::floor(text, offset).utf8_offset();
        for (start, word) in text.unicode_word_indices() {
            let end = start + word.len();
            if (start..end).contains(&offset) || (offset == text.len() && end == offset) {
                return (start, end);
            }
        }
        let probe = offset.min(text.len().saturating_sub(1));
        text.grapheme_indices(true)
            .find_map(|(start, grapheme)| {
                let end = start + grapheme.len();
                (probe >= start && probe < end).then_some((start, end))
            })
            .unwrap_or((offset, offset))
    }

    fn paragraph_range(text: &str, offset: usize) -> (usize, usize) {
        let offset = TextPosition::floor(text, offset).utf8_offset();
        let start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
        let end = text[offset..]
            .find('\n')
            .map_or(text.len(), |index| offset + index + 1);
        (start, end)
    }

    fn target_at_point(ctx: &ControllerContext, point: LayoutPoint) -> Option<SelectionTarget> {
        let hit = crate::hit_test::hit_test_with_viewports(
            ctx.ir,
            ctx.layout,
            ctx.scroll,
            ctx.viewport,
            point,
        )?;
        let mut current = Some(hit);
        let mut selectable: Option<(WidgetId, Semantics)> = None;
        while let Some(node_id) = current {
            let node = ctx.ir.nodes.get(&node_id)?;
            if let Op::Semantics(semantics) = &node.op {
                if let Some(region) = &semantics.selection_region {
                    if region.excluded {
                        return None;
                    }
                    if let Some((member_id, member_semantics)) = selectable {
                        return Some(SelectionTarget {
                            region_id: node_id,
                            member_id,
                            semantics: member_semantics,
                        });
                    }
                    let document = document_for_selection_owner(ctx.ir, node_id)?;
                    return Self::nearest_target(ctx, node_id, point, &document);
                }
                if selectable.is_none() && semantics.selectable_text && !semantics.disabled {
                    selectable = Some((node_id, semantics.clone()));
                }
            }
            current = node.parent;
        }
        selectable.map(|(member_id, semantics)| SelectionTarget {
            region_id: member_id,
            member_id,
            semantics,
        })
    }

    fn target_for_active_region(
        ctx: &ControllerContext,
        region_id: WidgetId,
        point: LayoutPoint,
        document: &RegionDocument,
    ) -> Option<SelectionTarget> {
        if let Some(target) = Self::target_at_point(ctx, point) {
            if target.region_id == region_id {
                return Some(target);
            }
        }
        Self::nearest_target(ctx, region_id, point, document)
    }

    fn nearest_target(
        ctx: &ControllerContext,
        region_id: WidgetId,
        point: LayoutPoint,
        document: &RegionDocument,
    ) -> Option<SelectionTarget> {
        document
            .members
            .iter()
            .filter_map(|member| {
                let semantics = crate::selection::selectable_semantics(ctx.ir, member.node_id)?;
                let (_, geometry) = Self::layout_geometry(ctx, member.node_id)?;
                let rect = geometry.rect;
                let max_x = rect.origin.x + rect.size.width;
                let max_y = rect.origin.y + rect.size.height;
                let dx = if point.x < rect.origin.x {
                    rect.origin.x - point.x
                } else if point.x > max_x {
                    point.x - max_x
                } else {
                    0.0
                };
                let dy = if point.y < rect.origin.y {
                    rect.origin.y - point.y
                } else if point.y > max_y {
                    point.y - max_y
                } else {
                    0.0
                };
                Some((dx * dx + dy * dy, member.node_id, semantics.clone()))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, member_id, semantics)| SelectionTarget {
                region_id,
                member_id,
                semantics,
            })
    }

    fn toolbar_action_hit(
        ir: &fission_ir::CoreIR,
        owner: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextContextMenuAction> {
        [
            TextContextMenuAction::Copy,
            TextContextMenuAction::Cut,
            TextContextMenuAction::Paste,
            TextContextMenuAction::SelectAll,
        ]
        .into_iter()
        .find(|action| {
            Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                text_context_menu_button_id(owner, *action),
            )
        })
    }

    fn selection_handle_hit(
        ir: &fission_ir::CoreIR,
        owner: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextSelectionHandleKind> {
        [
            TextSelectionHandleKind::Caret,
            TextSelectionHandleKind::Start,
            TextSelectionHandleKind::End,
        ]
        .into_iter()
        .find(|kind| {
            Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                selection_region_handle_id(owner, *kind),
            )
        })
    }

    fn execute_action(
        &mut self,
        ctx: &mut ControllerContext,
        owner: WidgetId,
        action: TextContextMenuAction,
    ) -> bool {
        let command = match action {
            TextContextMenuAction::Copy => Some(EditingCommand::Copy),
            TextContextMenuAction::SelectAll => Some(EditingCommand::SelectAll),
            TextContextMenuAction::Cut | TextContextMenuAction::Paste => None,
        };
        let handled = command.is_none_or(|command| {
            let old_focus = ctx.interaction.focused;
            ctx.interaction.set_focused(Some(owner));
            let handled = self.handle_editing_command(ctx, &command);
            ctx.interaction.set_focused(old_focus);
            handled
        });
        ctx.context_menu.close();
        handled
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
        if let Some((paragraph_id, paragraph_geom, paragraph)) =
            Self::resolved_paragraph_geometry(ctx, owner, value.len())
        {
            let local = Self::local_point(ctx, paragraph_id, paragraph_geom, point);
            return TextPosition::floor(value, paragraph.hit_test(local).min(value.len()))
                .utf8_offset();
        }
        let Some(measurer) = ctx.measurer else {
            return 0;
        };
        let width = (geom.rect.size.width > 0.0).then_some(geom.rect.size.width);
        let caret = if let Some(runs) = Self::rich_runs(ctx.ir, owner) {
            measurer.hit_test_rich(&runs, width, local.x, local.y)
        } else {
            measurer.hit_test(
                value,
                Self::font_size(ctx.ir, owner).unwrap_or(14.0),
                width,
                local.x,
                local.y,
            )
        };
        TextPosition::floor(value, caret.min(value.len())).utf8_offset()
    }

    fn resolved_paragraph_geometry<'a>(
        ctx: &'a ControllerContext,
        owner: WidgetId,
        expected_len: usize,
    ) -> Option<(
        WidgetId,
        &'a LayoutNodeGeometry,
        &'a fission_layout::ResolvedParagraphLayout,
    )> {
        fn walk<'a>(
            ctx: &'a ControllerContext,
            node_id: WidgetId,
            expected_len: usize,
        ) -> Option<(
            WidgetId,
            &'a LayoutNodeGeometry,
            &'a fission_layout::ResolvedParagraphLayout,
        )> {
            if let (Some(geometry), Some(paragraph)) = (
                ctx.layout.get_node_geometry(node_id),
                ctx.layout.get_resolved_paragraph(node_id),
            ) {
                let text_len = paragraph
                    .caret_stops
                    .iter()
                    .map(|stop| stop.index)
                    .max()
                    .unwrap_or(0);
                if text_len == expected_len {
                    return Some((node_id, geometry, paragraph));
                }
            }
            for child in &ctx.ir.nodes.get(&node_id)?.children {
                if let Some(found) = walk(ctx, *child, expected_len) {
                    return Some(found);
                }
            }
            None
        }
        walk(ctx, owner, expected_len)
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

    fn active_region(ctx: &ControllerContext) -> Option<WidgetId> {
        ctx.selectable_text
            .regions
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

    fn distance(left: LayoutPoint, right: LayoutPoint) -> f32 {
        let dx = left.x - right.x;
        let dy = left.y - right.y;
        (dx * dx + dy * dy).sqrt()
    }

    fn has_shift(modifiers: u8) -> bool {
        (modifiers & crate::event::MOD_SHIFT) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::SelectableTextController;

    #[test]
    fn word_selection_handles_unicode_and_whitespace() {
        assert_eq!(
            SelectableTextController::word_range("hello café", 8),
            (6, 11)
        );
        assert_eq!(SelectableTextController::word_range("one two", 3), (3, 4));
    }

    #[test]
    fn paragraph_selection_includes_the_trailing_separator() {
        assert_eq!(
            SelectableTextController::paragraph_range("one\ntwo\nthree", 5),
            (4, 8)
        );
        assert_eq!(
            SelectableTextController::paragraph_range("one\ntwo\nthree", 10),
            (8, 13)
        );
    }
}
