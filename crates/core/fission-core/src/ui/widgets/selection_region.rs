use crate::env::TextSelectionHandleKind;
use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::selection::{selectable_members_in_subtree, SelectionRegionController};
use crate::ui::widgets::context_menu::{
    anchor_to_local, text_context_menu_item_widget, text_context_menu_overlay_widget,
    TextContextMenuAction, TextContextMenuConfig,
};
use crate::ui::widgets::text_input::{TextMagnifierConfiguration, TextSelectionControls};
use crate::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Positioned, Row, Spacer, Text, Widget,
};
use fission_ir::{
    op::{Color, Fill},
    LayoutOp, Op, Role, SelectionRegionSemantics, Semantics, WidgetId,
};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

/// Platform presentation requested for selection handles, menus, and magnifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionPlatformStyle {
    /// Resolve from the active target and design system.
    #[default]
    Adaptive,
    Desktop,
    Mobile,
}

impl SelectionPlatformStyle {
    pub(crate) fn uses_touch_affordances(self, pointer: crate::event::PointerKind) -> bool {
        match self {
            Self::Desktop => false,
            Self::Mobile => true,
            Self::Adaptive => {
                cfg!(any(target_os = "android", target_os = "ios"))
                    || matches!(
                        pointer,
                        crate::event::PointerKind::Touch | crate::event::PointerKind::Stylus
                    )
            }
        }
    }
}

/// Interaction and platform-presentation policy for a coordinated selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectionRegionControls {
    pub word_selection_on_double_click: bool,
    pub paragraph_selection_on_triple_click: bool,
    pub word_selection_on_long_press: bool,
    pub platform_style: SelectionPlatformStyle,
    pub context_menu: TextContextMenuConfig,
    /// Minimum touch/stylus movement before a drag changes the selection.
    pub touch_slop: f32,
    /// Whether selection drags near a scroll viewport edge advance that viewport.
    pub edge_auto_scroll: bool,
    /// Distance from a viewport edge that activates automatic scrolling.
    pub edge_auto_scroll_threshold: f32,
    /// Logical pixels advanced for each pointer-move near an edge.
    pub edge_auto_scroll_step: f32,
    pub selection_controls: TextSelectionControls,
    pub magnifier_configuration: TextMagnifierConfiguration,
}

impl Default for SelectionRegionControls {
    fn default() -> Self {
        Self {
            word_selection_on_double_click: true,
            paragraph_selection_on_triple_click: true,
            word_selection_on_long_press: true,
            platform_style: SelectionPlatformStyle::Adaptive,
            context_menu: TextContextMenuConfig::read_only(),
            touch_slop: 8.0,
            edge_auto_scroll: true,
            edge_auto_scroll_threshold: 28.0,
            edge_auto_scroll_step: 18.0,
            selection_controls: TextSelectionControls::default(),
            magnifier_configuration: TextMagnifierConfiguration::default(),
        }
    }
}

/// Coordinates read-only selection across all selectable text descendants.
///
/// A `Text` or `RichText` with `selectable(true)` remains independently
/// selectable when no explicit region surrounds it. Wrap several such widgets
/// in a `SelectionRegion` to make dragging, copying, and select-all operate on
/// their retained document order as one selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectionRegion {
    pub id: Option<WidgetId>,
    pub child: Widget,
    /// Text inserted between descendants when copying the coordinated value.
    pub separator: String,
    pub controls: SelectionRegionControls,
    /// An excluded region blocks selection inherited from an ancestor region.
    pub excluded: bool,
}

impl SelectionRegion {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            id: None,
            child: child.into(),
            separator: "\n".into(),
            controls: SelectionRegionControls::default(),
            excluded: false,
        }
    }

    /// Excludes an otherwise selectable subtree from its nearest ancestor region.
    pub fn exclude(child: impl Into<Widget>) -> Self {
        Self {
            excluded: true,
            controls: SelectionRegionControls {
                context_menu: TextContextMenuConfig::disabled(),
                ..SelectionRegionControls::default()
            },
            ..Self::new(child)
        }
    }

    pub fn controller(mut self, controller: SelectionRegionController) -> Self {
        self.id = Some(controller.id());
        self
    }

    pub fn separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    pub fn controls(mut self, controls: SelectionRegionControls) -> Self {
        self.controls = controls;
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SelectionRegionRuntimeConfig {
    pub controls: SelectionRegionControls,
}

pub(crate) fn region_runtime_config(
    ir: &fission_ir::CoreIR,
    region_id: WidgetId,
) -> Option<&SelectionRegionRuntimeConfig> {
    ir.custom_render_objects
        .get(&region_id)?
        .downcast_ref::<SelectionRegionRuntimeConfig>()
}

pub(crate) fn selection_region_handle_id(
    region_id: WidgetId,
    kind: TextSelectionHandleKind,
) -> WidgetId {
    let suffix = match kind {
        TextSelectionHandleKind::Caret => 0,
        TextSelectionHandleKind::Start => 1,
        TextSelectionHandleKind::End => 2,
    };
    WidgetId::derived(region_id.as_u128(), &[0x5E1E, suffix])
}

pub(crate) fn selection_region_magnifier_id(region_id: WidgetId) -> WidgetId {
    WidgetId::derived(region_id.as_u128(), &[0x5E1E, 3])
}

pub(crate) fn selection_region_handle_position_id(
    region_id: WidgetId,
    kind: TextSelectionHandleKind,
) -> WidgetId {
    let suffix = match kind {
        TextSelectionHandleKind::Caret => 10,
        TextSelectionHandleKind::Start => 11,
        TextSelectionHandleKind::End => 12,
    };
    WidgetId::derived(region_id.as_u128(), &[0x5E1E, suffix])
}

fn build_selection_handle(
    cx: &mut InternalLoweringCx,
    region_id: WidgetId,
    controls: &TextSelectionControls,
    kind: TextSelectionHandleKind,
    point: fission_layout::LayoutPoint,
) -> WidgetId {
    let diameter = controls.handle_radius * 2.0;
    let handle: Widget = Button {
        id: Some(selection_region_handle_id(region_id, kind).into()),
        semantics: Some(Semantics {
            role: Role::Generic,
            draggable: true,
            ..Semantics::default()
        }),
        child: Some(
            Container::new(Spacer {
                width: Some(diameter),
                height: Some(diameter),
                ..Default::default()
            })
            .bg_fill(Fill::Solid(controls.handle_fill))
            .border(
                controls.handle_stroke.unwrap_or(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                }),
                controls.handle_stroke_width,
            )
            .border_radius(controls.handle_radius)
            .into(),
        ),
        width: Some(diameter),
        height: Some(diameter),
        padding: Some([0.0; 4]),
        content_align: ButtonContentAlign::Center,
        variant: ButtonVariant::Ghost,
        ..Default::default()
    }
    .into();
    Positioned {
        id: Some(selection_region_handle_position_id(region_id, kind)),
        left: Some((point.x - controls.handle_radius).max(0.0)),
        top: Some((point.y - controls.handle_radius).max(0.0)),
        width: Some(diameter),
        height: Some(diameter),
        child: Some(handle),
        ..Default::default()
    }
    .lower(cx)
}

fn magnifier_snippet(text: &str, offset: usize) -> String {
    let graphemes = text.grapheme_indices(true).collect::<Vec<_>>();
    if graphemes.is_empty() {
        return String::new();
    }
    let center = graphemes
        .iter()
        .position(|(index, _)| *index >= offset.min(text.len()))
        .unwrap_or(graphemes.len().saturating_sub(1));
    graphemes[center.saturating_sub(4)..(center + 5).min(graphemes.len())]
        .iter()
        .map(|(_, grapheme)| *grapheme)
        .collect()
}

fn build_magnifier(
    cx: &mut InternalLoweringCx,
    region_id: WidgetId,
    config: &TextMagnifierConfiguration,
    anchor: fission_layout::LayoutPoint,
    text: &str,
    offset: usize,
) -> WidgetId {
    let tokens = &cx.env.theme.tokens;
    let preview = Text::new(magnifier_snippet(text, offset))
        .size(tokens.typography.body_medium_size * config.scale)
        .color(tokens.colors.text_primary);
    let magnifier: Widget = Container::new(preview)
        .width(config.diameter)
        .height(config.diameter)
        .bg_fill(Fill::Solid(tokens.colors.surface))
        .border(
            config.border_color.unwrap_or(tokens.colors.border),
            config.border_width,
        )
        .border_radius(config.border_radius)
        .padding_all(8.0)
        .into();
    Positioned {
        id: Some(selection_region_magnifier_id(region_id)),
        left: Some((anchor.x - config.diameter * 0.5).max(0.0)),
        top: Some((anchor.y - config.diameter - 18.0).max(0.0)),
        width: Some(config.diameter),
        height: Some(config.diameter),
        child: Some(magnifier),
        ..Default::default()
    }
    .lower(cx)
}

fn build_mobile_toolbar(
    config: &TextContextMenuConfig,
    owner: WidgetId,
    anchor: fission_layout::LayoutPoint,
    selection_present: bool,
    document_present: bool,
) -> Widget {
    let actions = config
        .actions
        .iter()
        .copied()
        .map(|action| {
            let enabled = match action {
                TextContextMenuAction::Copy => selection_present,
                TextContextMenuAction::SelectAll => document_present,
                TextContextMenuAction::Cut | TextContextMenuAction::Paste => false,
            };
            text_context_menu_item_widget(owner, action, enabled)
        })
        .collect();
    let background = config.menu.background.unwrap_or(fission_ir::op::Color {
        r: 255,
        g: 255,
        b: 255,
        a: 248,
    });
    let border = config.menu.border_color.unwrap_or(fission_ir::op::Color {
        r: 226,
        g: 232,
        b: 240,
        a: 255,
    });
    Positioned {
        left: Some(anchor.x.max(0.0)),
        top: Some((anchor.y - 48.0).max(0.0)),
        child: Some(
            Container::new(Row {
                children: actions,
                gap: Some(config.menu.gap),
                ..Default::default()
            })
            .padding(config.menu.padding)
            .bg(background)
            .border(border, config.menu.border_width)
            .border_radius(config.menu.border_radius)
            .shadow(config.menu.shadow.unwrap_or(fission_ir::op::BoxShadow {
                spread_radius: 0.0,
                inset: false,
                offset: (0.0, 8.0),
                blur_radius: 24.0,
                color: fission_ir::op::Color {
                    r: 15,
                    g: 23,
                    b: 42,
                    a: 38,
                },
            }))
            .into(),
        ),
        ..Default::default()
    }
    .into()
}

pub(crate) fn wrap_implicit_selection_affordances(
    cx: &mut InternalLoweringCx<'_>,
    owner: WidgetId,
    visual_id: WidgetId,
    context_menu: &TextContextMenuConfig,
    selection: Option<(usize, usize)>,
    text: &str,
) -> WidgetId {
    let controls = SelectionRegionControls {
        context_menu: context_menu.clone(),
        ..SelectionRegionControls::default()
    };
    let state = cx
        .runtime_state
        .selectable_text
        .region(owner)
        .cloned()
        .unwrap_or_default();
    let touch = controls
        .platform_style
        .uses_touch_affordances(state.pointer_kind);
    let mut overlays = Vec::new();
    if touch && controls.selection_controls.enabled {
        if selection.is_some() {
            for (kind, point) in [
                (TextSelectionHandleKind::Start, state.selection_start_handle),
                (TextSelectionHandleKind::End, state.selection_end_handle),
            ] {
                if let Some(point) = point {
                    overlays.push(build_selection_handle(
                        cx,
                        owner,
                        &controls.selection_controls,
                        kind,
                        point,
                    ));
                }
            }
        } else if controls.selection_controls.show_collapsed_handle {
            if let Some(point) = state.caret_handle {
                overlays.push(build_selection_handle(
                    cx,
                    owner,
                    &controls.selection_controls,
                    TextSelectionHandleKind::Caret,
                    point,
                ));
            }
        }
    }
    if touch && controls.magnifier_configuration.enabled && state.magnifier_visible {
        if let (Some(anchor), Some(region_selection)) = (
            state.magnifier_anchor,
            cx.runtime_state.selectable_text.region_selection(owner),
        ) {
            overlays.push(build_magnifier(
                cx,
                owner,
                &controls.magnifier_configuration,
                anchor,
                text,
                region_selection.extent.offset.utf8_offset(),
            ));
        }
    }
    if context_menu.enabled && cx.runtime_state.context_menu.owner == Some(owner) {
        let anchor = cx
            .runtime_state
            .context_menu
            .anchor
            .map(|point| anchor_to_local(cx, owner, point))
            .unwrap_or_default();
        let menu = if touch {
            build_mobile_toolbar(
                context_menu,
                owner,
                anchor,
                selection.is_some(),
                !text.is_empty(),
            )
        } else {
            text_context_menu_overlay_widget(context_menu, owner, anchor, |action| match action {
                TextContextMenuAction::Copy => selection.is_some(),
                TextContextMenuAction::SelectAll => !text.is_empty(),
                TextContextMenuAction::Cut | TextContextMenuAction::Paste => false,
            })
        };
        overlays.push(menu.lower(cx));
    }
    if overlays.is_empty() {
        visual_id
    } else {
        let mut stack = InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::ZStack));
        stack.add_child(visual_id);
        for overlay in overlays {
            stack.add_child(overlay);
        }
        stack.build(cx)
    }
}

impl InternalLower for SelectionRegion {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let owner = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(owner);
        let child_id = self.child.lower(cx);
        let member_ids = selectable_members_in_subtree(&cx.ir, child_id);

        let document = member_ids
            .iter()
            .filter_map(|id| {
                cx.ir.nodes.get(id).and_then(|node| match &node.op {
                    Op::Semantics(semantics) => semantics.value.as_deref(),
                    _ => None,
                })
            })
            .collect::<Vec<_>>()
            .join(&self.separator);
        let runtime_selection = cx.runtime_state.selectable_text.region_selection(owner);
        let accessibility_selection = runtime_selection.and_then(|selection| {
            let mut offset = 0;
            let mut base = None;
            let mut extent = None;
            for (index, member_id) in member_ids.iter().enumerate() {
                if index > 0 {
                    offset += self.separator.len();
                }
                let value_len = cx
                    .ir
                    .nodes
                    .get(member_id)
                    .and_then(|node| match &node.op {
                        Op::Semantics(semantics) => semantics.value.as_ref(),
                        _ => None,
                    })
                    .map_or(0, String::len);
                if selection.base.node_id == *member_id {
                    base = Some(offset + selection.base.offset.utf8_offset().min(value_len));
                }
                if selection.extent.node_id == *member_id {
                    extent = Some(offset + selection.extent.offset.utf8_offset().min(value_len));
                }
                offset += value_len;
            }
            Some((base?, extent?))
        });

        let selection_present =
            runtime_selection.is_some_and(|selection| !selection.is_collapsed());
        let region_state = cx
            .runtime_state
            .selectable_text
            .region(owner)
            .cloned()
            .unwrap_or_default();
        let touch_affordances = self
            .controls
            .platform_style
            .uses_touch_affordances(region_state.pointer_kind);
        let mut overlays = Vec::new();
        if !self.excluded && touch_affordances && self.controls.selection_controls.enabled {
            if selection_present {
                if let Some(point) = region_state.selection_start_handle {
                    overlays.push(build_selection_handle(
                        cx,
                        owner,
                        &self.controls.selection_controls,
                        TextSelectionHandleKind::Start,
                        point,
                    ));
                }
                if let Some(point) = region_state.selection_end_handle {
                    overlays.push(build_selection_handle(
                        cx,
                        owner,
                        &self.controls.selection_controls,
                        TextSelectionHandleKind::End,
                        point,
                    ));
                }
            } else if self.controls.selection_controls.show_collapsed_handle {
                if let Some(point) = region_state.caret_handle {
                    overlays.push(build_selection_handle(
                        cx,
                        owner,
                        &self.controls.selection_controls,
                        TextSelectionHandleKind::Caret,
                        point,
                    ));
                }
            }
        }
        if !self.excluded
            && touch_affordances
            && self.controls.magnifier_configuration.enabled
            && region_state.magnifier_visible
        {
            if let (Some(anchor), Some(selection)) =
                (region_state.magnifier_anchor, runtime_selection)
            {
                let member_text = cx
                    .ir
                    .nodes
                    .get(&selection.extent.node_id)
                    .and_then(|node| match &node.op {
                        Op::Semantics(semantics) => semantics.value.as_deref(),
                        _ => None,
                    })
                    .unwrap_or_default()
                    .to_owned();
                overlays.push(build_magnifier(
                    cx,
                    owner,
                    &self.controls.magnifier_configuration,
                    anchor,
                    &member_text,
                    selection.extent.offset.utf8_offset(),
                ));
            }
        }
        if !self.excluded
            && self.controls.context_menu.enabled
            && cx.runtime_state.context_menu.owner == Some(owner)
        {
            let anchor = cx
                .runtime_state
                .context_menu
                .anchor
                .map(|point| anchor_to_local(cx, owner, point))
                .unwrap_or_default();
            let menu = if touch_affordances {
                build_mobile_toolbar(
                    &self.controls.context_menu,
                    owner,
                    anchor,
                    selection_present,
                    !document.is_empty(),
                )
            } else {
                text_context_menu_overlay_widget(
                    &self.controls.context_menu,
                    owner,
                    anchor,
                    |action| match action {
                        TextContextMenuAction::Copy => selection_present,
                        TextContextMenuAction::SelectAll => !document.is_empty(),
                        TextContextMenuAction::Cut | TextContextMenuAction::Paste => false,
                    },
                )
            };
            overlays.push(menu.lower(cx));
        }
        let visual_id = if overlays.is_empty() {
            child_id
        } else {
            let mut stack = InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::ZStack));
            stack.add_child(child_id);
            for overlay in overlays {
                stack.add_child(overlay);
            }
            stack.build(cx)
        };

        let semantics = Semantics {
            role: if self.excluded {
                Role::Generic
            } else {
                Role::Text
            },
            value: (!self.excluded).then_some(document),
            focusable: !self.excluded && !member_ids.is_empty(),
            read_only: !self.excluded,
            multiline: member_ids.len() > 1,
            text_selection: accessibility_selection,
            context_menu: !self.excluded && self.controls.context_menu.enabled,
            selection_region: Some(SelectionRegionSemantics {
                excluded: self.excluded,
                separator: self.separator.clone(),
            }),
            ..Semantics::default()
        };
        let mut builder = InternalIrBuilder::new(owner, Op::Semantics(semantics));
        builder.add_child(visual_id);
        let owner = builder.build(cx);
        cx.ir.custom_render_objects.insert(
            owner,
            std::sync::Arc::new(SelectionRegionRuntimeConfig {
                controls: self.controls.clone(),
            }),
        );
        cx.pop_scope();
        owner
    }
}
