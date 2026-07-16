use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::{Button, ButtonVariant, Column, Container, Positioned, Text, TextContent, Widget};
use crate::ActionEnvelope;
use fission_ir::{
    op::{BoxShadow, Color, LayoutOp, Op},
    FocusPolicy, Semantics, WidgetId,
};
use serde::{Deserialize, Serialize};

/// A popup menu shown for a secondary click or other contextual activation.
///
/// Menu items accept arbitrary child widgets, so applications can provide plain
/// translated text, icons, shortcuts, badges, or richer branded rows without
/// hard-coding menu strings in the framework.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenu {
    /// Entries rendered in order from top to bottom.
    pub items: Vec<ContextMenuEntry>,
    /// Fixed popup width. When omitted, [`ContextMenu::min_width`] is used.
    pub width: Option<f32>,
    /// Minimum popup width used by the default layout.
    pub min_width: f32,
    /// Maximum popup width before child content wraps or clips according to its own layout.
    pub max_width: Option<f32>,
    /// Popup interior padding `[left, right, top, bottom]`.
    pub padding: [f32; 4],
    /// Gap between menu rows.
    pub gap: f32,
    /// Popup corner radius.
    pub border_radius: f32,
    /// Optional popup background colour. Defaults to the framework surface colour.
    pub background: Option<Color>,
    /// Optional border colour. Defaults to a subtle neutral border.
    pub border_color: Option<Color>,
    /// Border width in logical pixels.
    pub border_width: f32,
    /// Optional popup shadow.
    pub shadow: Option<BoxShadow>,
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            width: None,
            min_width: 176.0,
            max_width: Some(320.0),
            padding: [8.0, 8.0, 8.0, 8.0],
            gap: 4.0,
            border_radius: 12.0,
            background: None,
            border_color: None,
            border_width: 1.0,
            shadow: Some(BoxShadow {
                offset: (0.0, 12.0),
                blur_radius: 28.0,
                color: Color {
                    r: 15,
                    g: 23,
                    b: 42,
                    a: 52,
                },
            }),
        }
    }
}

impl ContextMenu {
    pub fn with_items(items: impl IntoIterator<Item = ContextMenuEntry>) -> Self {
        Self {
            items: items.into_iter().collect(),
            ..Default::default()
        }
    }

    pub(crate) fn overlay_widget(
        &self,
        owner: WidgetId,
        anchor: fission_layout::LayoutPoint,
    ) -> Widget {
        let children = self
            .items
            .iter()
            .enumerate()
            .map(|(index, entry)| entry.widget(owner, index))
            .collect();

        let background = self.background.unwrap_or(Color {
            r: 255,
            g: 255,
            b: 255,
            a: 248,
        });
        let border = self.border_color.unwrap_or(Color {
            r: 226,
            g: 232,
            b: 240,
            a: 255,
        });

        Positioned {
            id: Some(context_menu_popup_id(owner)),
            left: Some(anchor.x),
            top: Some(anchor.y),
            child: Some(
                Container::new(Column {
                    id: Some(WidgetId::derived(owner.as_u128(), &[0xC0A7, 2])),
                    children,
                    gap: Some(self.gap),
                    ..Default::default()
                })
                .width(self.width.unwrap_or(self.min_width))
                .max_width(self.max_width.unwrap_or(320.0))
                .padding(self.padding)
                .bg(background)
                .border(border, self.border_width)
                .border_radius(self.border_radius)
                .shadow(self.shadow.unwrap_or(BoxShadow {
                    offset: (0.0, 8.0),
                    blur_radius: 24.0,
                    color: Color {
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
}

/// A row inside a [`ContextMenu`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextMenuEntry {
    /// A selectable menu row.
    Item(ContextMenuItem),
    /// A one-pixel visual separator between groups.
    Separator,
}

impl ContextMenuEntry {
    fn widget(&self, owner: WidgetId, index: usize) -> Widget {
        match self {
            Self::Item(item) => item.widget(owner, index),
            Self::Separator => Container {
                id: Some(context_menu_entry_id(owner, index)),
                child: Some(Text::new("").into()),
                height: Some(1.0),
                background_color: Some(Color {
                    r: 226,
                    g: 232,
                    b: 240,
                    a: 255,
                }),
                background_fill: Some(fission_ir::op::Fill::Solid(Color {
                    r: 226,
                    g: 232,
                    b: 240,
                    a: 255,
                })),
                ..Default::default()
            }
            .into(),
        }
    }
}

/// A selectable context menu item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuItem {
    /// Stable id used to derive the row widget identity.
    pub id: String,
    /// The visible row content. Use `TextContent::Key` or `KeyWithFallback` for i18n.
    pub child: Widget,
    /// Action dispatched when the row is selected.
    pub on_select: Option<ActionEnvelope>,
    /// Whether the row can be activated.
    pub enabled: bool,
    /// Whether selecting the row should close the menu. Reserved for shells that support persistent menus.
    pub close_on_select: bool,
    /// Optional accessible label when the child is not self-describing.
    pub semantics_label: Option<String>,
}

impl ContextMenuItem {
    pub fn new(id: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            id: id.into(),
            child: child.into(),
            on_select: None,
            enabled: true,
            close_on_select: true,
            semantics_label: None,
        }
    }

    pub fn text(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, Text::new(label.into()))
    }

    pub fn text_key(
        id: impl Into<String>,
        key: impl Into<String>,
        fallback: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            Text::new(TextContent::KeyWithFallback {
                key: key.into(),
                fallback: fallback.into(),
            }),
        )
    }

    pub fn on_select(mut self, action: ActionEnvelope) -> Self {
        self.on_select = Some(action);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics_label = Some(label.into());
        self
    }

    fn widget(&self, owner: WidgetId, index: usize) -> Widget {
        let semantics = Semantics {
            role: fission_ir::Role::Button,
            label: self.semantics_label.clone(),
            focusable: true,
            disabled: !self.enabled,
            ..Semantics::default()
        };

        Button {
            id: Some(
                context_menu_item_id(owner, &self.id)
                    .unwrap_or_else(|| context_menu_entry_id(owner, index)),
            ),
            child: Some(self.child.clone()),
            on_press: self.on_select.clone().filter(|_| self.enabled),
            semantics: Some(semantics),
            focus_policy: FocusPolicy::PreserveCurrentOnPointer,
            variant: ButtonVariant::Ghost,
            padding: Some([10.0, 10.0, 8.0, 8.0]),
            disabled: !self.enabled,
            ..Default::default()
        }
        .into()
    }
}

pub(crate) fn text_context_menu_overlay_widget(
    config: &TextContextMenuConfig,
    owner: WidgetId,
    anchor: fission_layout::LayoutPoint,
    action_enabled: impl Fn(TextContextMenuAction) -> bool,
) -> Widget {
    let menu = config.menu.clone();
    let children = config
        .actions
        .iter()
        .copied()
        .map(|action| text_context_menu_item_widget(owner, action, action_enabled(action)))
        .collect();

    let background = menu.background.unwrap_or(Color {
        r: 255,
        g: 255,
        b: 255,
        a: 248,
    });
    let border = menu.border_color.unwrap_or(Color {
        r: 226,
        g: 232,
        b: 240,
        a: 255,
    });

    Positioned {
        id: Some(context_menu_popup_id(owner)),
        left: Some(anchor.x),
        top: Some(anchor.y),
        child: Some(
            Container::new(Column {
                id: Some(WidgetId::derived(owner.as_u128(), &[0xC0A7, 4])),
                children,
                gap: Some(menu.gap),
                ..Default::default()
            })
            .width(menu.width.unwrap_or(menu.min_width))
            .max_width(menu.max_width.unwrap_or(320.0))
            .padding(menu.padding)
            .bg(background)
            .border(border, menu.border_width)
            .border_radius(menu.border_radius)
            .shadow(menu.shadow.unwrap_or(BoxShadow {
                offset: (0.0, 8.0),
                blur_radius: 24.0,
                color: Color {
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

/// Widget wrapper that gives any subtree a context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuRegion {
    /// Stable owner id for context-menu open state.
    pub id: Option<WidgetId>,
    /// Subtree that can open the menu on secondary click.
    pub child: Widget,
    /// Menu rendered when this region is active.
    pub menu: ContextMenu,
    /// Whether the region responds to secondary click.
    pub enabled: bool,
    /// Optional semantics for the owner node. Fission sets `context_menu` automatically.
    pub semantics: Option<Semantics>,
}

impl ContextMenuRegion {
    pub fn new(child: impl Into<Widget>, menu: ContextMenu) -> Self {
        Self {
            id: None,
            child: child.into(),
            menu,
            enabled: true,
            semantics: None,
        }
    }

    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        let semantics = self.semantics.get_or_insert_with(Semantics::default);
        semantics.identifier = Some(identifier.into());
        self
    }
}

impl InternalLower for ContextMenuRegion {
    fn lower(&self, cx: &mut InternalLoweringCx<'_>) -> WidgetId {
        let owner = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(owner);

        let child_id = self.child.lower(cx);
        let visual_id = if self.enabled && cx.runtime_state.context_menu.owner == Some(owner) {
            let anchor = cx
                .runtime_state
                .context_menu
                .anchor
                .map(|screen_anchor| anchor_to_local(cx, owner, screen_anchor))
                .unwrap_or_else(|| fission_layout::LayoutPoint::new(0.0, 0.0));
            let menu_id = self.menu.overlay_widget(owner, anchor).lower(cx);
            let mut stack = InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::ZStack));
            stack.add_child(child_id);
            stack.add_child(menu_id);
            stack.build(cx)
        } else {
            child_id
        };

        let mut semantics = self.semantics.clone().unwrap_or_default();
        semantics.context_menu = self.enabled;
        let mut builder = InternalIrBuilder::new(owner, Op::Semantics(semantics));
        builder.add_child(visual_id);
        cx.pop_scope();
        builder.build(cx)
    }
}

/// Standard editing/selection commands used by built-in text context menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextContextMenuAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
}

impl TextContextMenuAction {
    pub fn fallback_label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Copy => "fission.context_menu.copy",
            Self::Cut => "fission.context_menu.cut",
            Self::Paste => "fission.context_menu.paste",
            Self::SelectAll => "fission.context_menu.select_all",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContextMenuConfig {
    /// Whether the built-in text context menu is enabled.
    pub enabled: bool,
    /// Standard text actions shown in order. Unsupported actions are rendered disabled.
    pub actions: Vec<TextContextMenuAction>,
    /// Visual menu configuration used by the built-in text menu.
    pub menu: ContextMenu,
}

impl TextContextMenuConfig {
    pub fn read_only() -> Self {
        Self {
            enabled: true,
            actions: vec![
                TextContextMenuAction::Copy,
                TextContextMenuAction::SelectAll,
            ],
            menu: ContextMenu::default(),
        }
    }

    pub fn editing() -> Self {
        Self {
            enabled: true,
            actions: vec![
                TextContextMenuAction::Copy,
                TextContextMenuAction::Cut,
                TextContextMenuAction::Paste,
                TextContextMenuAction::SelectAll,
            ],
            menu: ContextMenu::default(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::read_only()
        }
    }
}

impl Default for TextContextMenuConfig {
    fn default() -> Self {
        Self::read_only()
    }
}

pub(crate) fn text_context_menu_item_widget(
    owner: WidgetId,
    action: TextContextMenuAction,
    enabled: bool,
) -> Widget {
    let child = Text::new(TextContent::KeyWithFallback {
        key: action.label_key().to_string(),
        fallback: action.fallback_label().to_string(),
    });

    Button {
        id: Some(text_context_menu_button_id(owner, action)),
        child: Some(child.into()),
        semantics: Some(Semantics {
            role: fission_ir::Role::Button,
            label: Some(action.fallback_label().to_string()),
            focusable: true,
            disabled: !enabled,
            ..Semantics::default()
        }),
        focus_policy: FocusPolicy::PreserveCurrentOnPointer,
        variant: ButtonVariant::Ghost,
        padding: Some([10.0, 10.0, 8.0, 8.0]),
        disabled: !enabled,
        ..Default::default()
    }
    .into()
}

pub(crate) fn action_index(action: TextContextMenuAction) -> usize {
    match action {
        TextContextMenuAction::Copy => 0,
        TextContextMenuAction::Cut => 1,
        TextContextMenuAction::Paste => 2,
        TextContextMenuAction::SelectAll => 3,
    }
}

pub(crate) fn context_menu_popup_id(owner: WidgetId) -> WidgetId {
    WidgetId::derived(owner.as_u128(), &[0xC0A7, 0])
}

pub(crate) fn context_menu_entry_id(owner: WidgetId, index: usize) -> WidgetId {
    WidgetId::derived(owner.as_u128(), &[0xC0A7, 1, index as u32])
}

pub(crate) fn context_menu_item_id(owner: WidgetId, id: &str) -> Option<WidgetId> {
    if id.is_empty() {
        None
    } else {
        Some(WidgetId::explicit(&format!(
            "fission.context_menu.{owner}.{id}"
        )))
    }
}

pub(crate) fn text_context_menu_button_id(
    owner: WidgetId,
    action: TextContextMenuAction,
) -> WidgetId {
    WidgetId::derived(owner.as_u128(), &[0xC0A7, 3, action_index(action) as u32])
}

pub(crate) fn anchor_to_local(
    cx: &InternalLoweringCx<'_>,
    owner: WidgetId,
    screen_anchor: fission_layout::LayoutPoint,
) -> fission_layout::LayoutPoint {
    let Some(layout) = cx.layout else {
        return screen_anchor;
    };
    let Some(rect) = layout.get_node_rect(owner) else {
        return screen_anchor;
    };
    fission_layout::LayoutPoint::new(
        (screen_anchor.x - rect.origin.x).max(0.0),
        (screen_anchor.y - rect.origin.y).max(0.0),
    )
}
