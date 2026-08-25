use super::custom_render::CustomRenderObject;
use super::traits::{InternalLower, InternalLowerer};
#[cfg(feature = "interactive-canvas")]
use super::widgets::InteractiveViewer;
use super::widgets::{
    ActionScope, Align, Button, Checkbox, Clip, Column, Composite, Container, ContextMenuEntry,
    ContextMenuRegion, FocusScope, GestureDetector, Grid, GridItem, Icon, Image, LazyColumn,
    Overlay, Positioned, Pressable, Radio, Responsive, RichText, Row, SafeArea, Scroll,
    SelectionRegion, SemanticsRegion, Slider, Spacer, Switch, Text, TextInput, Transform, Video,
    ZStack,
};
use crate::lowering::InternalLoweringCx;
use fission_ir::{Op, StructuralOp, WidgetId};
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Widget {
    kind: Box<WidgetKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WidgetKind {
    Identified {
        id: WidgetId,
        child: Widget,
    },
    ActionScope(ActionScope),
    Row(Row),
    Column(Column),
    Align(Align),
    FocusScope(FocusScope),
    SelectionRegion(SelectionRegion),
    Clip(Clip),
    Text(Text),
    RichText(RichText),
    Transform(Transform),
    #[cfg(feature = "interactive-canvas")]
    InteractiveViewer(InteractiveViewer),
    Button(Button),
    Pressable(Pressable),
    TextInput(TextInput),
    Scroll(Scroll),
    SemanticsRegion(SemanticsRegion),
    Image(Image),
    Video(Video),
    ZStack(ZStack),
    Overlay(Overlay),
    Container(Container),
    ContextMenuRegion(ContextMenuRegion),
    GestureDetector(GestureDetector),
    Grid(Grid),
    GridItem(GridItem),
    Responsive(Responsive),
    Checkbox(Checkbox),
    Switch(Switch),
    Radio(Radio),
    SafeArea(SafeArea),
    Positioned(Positioned),
    Spacer(Spacer),
    Slider(Slider),
    LazyColumn(LazyColumn),
    Icon(Icon),
    Composite(Composite),
    Custom(InternalRenderNode),
}

impl Widget {
    const CHILD_ROLE: u32 = 0xF155_2000;

    /// Returns the concrete kind represented by this type-erased widget.
    pub fn kind(&self) -> &WidgetKind {
        self.kind.as_ref()
    }

    /// Visits this widget and every descendant in depth-first order.
    ///
    /// Returning [`ControlFlow::Break`] stops traversal immediately. The
    /// visitor is read-only; callers that need to transform a tree should
    /// construct a new widget tree through the normal authoring API.
    pub fn visit(&self, visitor: &mut impl FnMut(&Widget) -> ControlFlow<()>) -> ControlFlow<()> {
        visitor(self)?;
        match self.kind.as_ref() {
            WidgetKind::Identified { child, .. }
            | WidgetKind::ActionScope(ActionScope { child, .. })
            | WidgetKind::SelectionRegion(SelectionRegion { child, .. })
            | WidgetKind::Align(Align { child, .. })
            | WidgetKind::Clip(Clip { child, .. })
            | WidgetKind::Transform(Transform { child, .. })
            | WidgetKind::Pressable(Pressable { child, .. })
            | WidgetKind::GestureDetector(GestureDetector { child, .. })
            | WidgetKind::GridItem(GridItem { child, .. })
            | WidgetKind::SafeArea(SafeArea { child, .. })
            | WidgetKind::Composite(Composite { child, .. }) => child.visit(visitor),
            WidgetKind::Row(Row { children, .. })
            | WidgetKind::Column(Column { children, .. })
            | WidgetKind::FocusScope(FocusScope { children, .. })
            | WidgetKind::ZStack(ZStack { children, .. })
            | WidgetKind::Grid(Grid { children, .. })
            | WidgetKind::LazyColumn(LazyColumn { children, .. }) => {
                for child in children {
                    child.visit(visitor)?;
                }
                ControlFlow::Continue(())
            }
            WidgetKind::Button(Button { child, .. })
            | WidgetKind::Scroll(Scroll { child, .. })
            | WidgetKind::SemanticsRegion(SemanticsRegion { child, .. })
            | WidgetKind::Container(Container { child, .. })
            | WidgetKind::Positioned(Positioned { child, .. }) => {
                if let Some(child) = child {
                    child.visit(visitor)?;
                }
                ControlFlow::Continue(())
            }
            WidgetKind::Overlay(Overlay {
                content, overlay, ..
            }) => {
                content.visit(visitor)?;
                overlay.visit(visitor)
            }
            WidgetKind::ContextMenuRegion(ContextMenuRegion { child, menu, .. }) => {
                child.visit(visitor)?;
                for entry in &menu.items {
                    if let ContextMenuEntry::Item(item) = entry {
                        item.child.visit(visitor)?;
                    }
                }
                ControlFlow::Continue(())
            }
            WidgetKind::Responsive(Responsive {
                cases, fallback, ..
            }) => {
                for case in cases {
                    case.child.visit(visitor)?;
                }
                fallback.visit(visitor)
            }
            WidgetKind::RichText(RichText { inline_widgets, .. }) => {
                for inline in inline_widgets {
                    inline.widget.visit(visitor)?;
                }
                ControlFlow::Continue(())
            }
            WidgetKind::TextInput(TextInput { prefix, suffix, .. }) => {
                if let Some(prefix) = prefix {
                    prefix.visit(visitor)?;
                }
                if let Some(suffix) = suffix {
                    suffix.visit(visitor)?;
                }
                ControlFlow::Continue(())
            }
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(InteractiveViewer { child, .. }) => child.visit(visitor),
            WidgetKind::Custom(_)
            | WidgetKind::Text(_)
            | WidgetKind::Image(_)
            | WidgetKind::Video(_)
            | WidgetKind::Checkbox(_)
            | WidgetKind::Switch(_)
            | WidgetKind::Radio(_)
            | WidgetKind::Spacer(_)
            | WidgetKind::Slider(_)
            | WidgetKind::Icon(_) => ControlFlow::Continue(()),
        }
    }

    pub(crate) fn with_id(self, id: WidgetId) -> Self {
        let kind = match *self.kind {
            WidgetKind::Identified { child, .. } => WidgetKind::Identified { id, child },
            WidgetKind::ActionScope(w) => WidgetKind::Identified {
                id,
                child: Widget {
                    kind: Box::new(WidgetKind::ActionScope(w)),
                },
            },
            WidgetKind::Custom(w) => WidgetKind::Identified {
                id,
                child: Widget {
                    kind: Box::new(WidgetKind::Custom(w)),
                },
            },
            WidgetKind::Row(mut w) => {
                w.id = Some(id);
                WidgetKind::Row(w)
            }
            WidgetKind::Column(mut w) => {
                w.id = Some(id);
                WidgetKind::Column(w)
            }
            WidgetKind::Align(mut w) => {
                w.id = Some(id);
                WidgetKind::Align(w)
            }
            WidgetKind::FocusScope(mut w) => {
                w.id = Some(id);
                WidgetKind::FocusScope(w)
            }
            WidgetKind::SelectionRegion(mut w) => {
                w.id = Some(id);
                WidgetKind::SelectionRegion(w)
            }
            WidgetKind::Clip(mut w) => {
                w.id = Some(id);
                WidgetKind::Clip(w)
            }
            WidgetKind::Text(mut w) => {
                w.id = Some(id);
                WidgetKind::Text(w)
            }
            WidgetKind::RichText(mut w) => {
                w.id = Some(id);
                WidgetKind::RichText(w)
            }
            WidgetKind::Transform(mut w) => {
                w.id = Some(id);
                WidgetKind::Transform(w)
            }
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(mut w) => {
                w.id = Some(id);
                WidgetKind::InteractiveViewer(w)
            }
            WidgetKind::Button(mut w) => {
                w.id = Some(id);
                WidgetKind::Button(w)
            }
            WidgetKind::Pressable(mut w) => {
                w.id = Some(id);
                WidgetKind::Pressable(w)
            }
            WidgetKind::TextInput(mut w) => {
                w.id = Some(id);
                WidgetKind::TextInput(w)
            }
            WidgetKind::Scroll(mut w) => {
                w.id = Some(id);
                WidgetKind::Scroll(w)
            }
            WidgetKind::SemanticsRegion(mut w) => {
                w.id = Some(id);
                WidgetKind::SemanticsRegion(w)
            }
            WidgetKind::Image(mut w) => {
                w.id = Some(id);
                WidgetKind::Image(w)
            }
            WidgetKind::Video(mut w) => {
                w.id = Some(id);
                WidgetKind::Video(w)
            }
            WidgetKind::ZStack(mut w) => {
                w.id = Some(id);
                WidgetKind::ZStack(w)
            }
            WidgetKind::Overlay(mut w) => {
                w.id = Some(id);
                WidgetKind::Overlay(w)
            }
            WidgetKind::Container(mut w) => {
                w.id = Some(id);
                WidgetKind::Container(w)
            }
            WidgetKind::ContextMenuRegion(mut w) => {
                w.id = Some(id);
                WidgetKind::ContextMenuRegion(w)
            }
            WidgetKind::GestureDetector(mut w) => {
                w.id = Some(id);
                WidgetKind::GestureDetector(w)
            }
            WidgetKind::Grid(mut w) => {
                w.id = Some(id);
                WidgetKind::Grid(w)
            }
            WidgetKind::GridItem(mut w) => {
                w.id = Some(id);
                WidgetKind::GridItem(w)
            }
            WidgetKind::Responsive(mut w) => {
                w.id = Some(id);
                WidgetKind::Responsive(w)
            }
            WidgetKind::Checkbox(mut w) => {
                w.id = Some(id);
                WidgetKind::Checkbox(w)
            }
            WidgetKind::Switch(mut w) => {
                w.id = Some(id);
                WidgetKind::Switch(w)
            }
            WidgetKind::Radio(mut w) => {
                w.id = Some(id);
                WidgetKind::Radio(w)
            }
            WidgetKind::SafeArea(mut w) => {
                w.id = Some(id);
                WidgetKind::SafeArea(w)
            }
            WidgetKind::Positioned(mut w) => {
                w.id = Some(id);
                WidgetKind::Positioned(w)
            }
            WidgetKind::Spacer(mut w) => {
                w.id = Some(id);
                WidgetKind::Spacer(w)
            }
            WidgetKind::Slider(mut w) => {
                w.id = Some(id);
                WidgetKind::Slider(w)
            }
            WidgetKind::LazyColumn(mut w) => {
                w.id = Some(id);
                WidgetKind::LazyColumn(w)
            }
            WidgetKind::Icon(mut w) => {
                w.id = Some(id);
                WidgetKind::Icon(w)
            }
            WidgetKind::Composite(mut w) => {
                w.id = Some(id);
                WidgetKind::Composite(w)
            }
        };
        Self {
            kind: Box::new(kind),
        }
    }

    pub fn id<I>(self, id: I) -> Self
    where
        I: Into<WidgetId>,
    {
        self.with_id(id.into())
    }

    pub(crate) fn custom(node: InternalRenderNode) -> Self {
        Self {
            kind: Box::new(WidgetKind::Custom(node)),
        }
    }

    pub(crate) fn from_pressable_raw(pressable: Pressable) -> Self {
        Self {
            kind: Box::new(WidgetKind::Pressable(pressable)),
        }
    }

    pub(crate) fn into_text(self) -> Result<Text, Self> {
        match *self.kind {
            WidgetKind::Text(text) => Ok(text),
            kind => Err(Self {
                kind: Box::new(kind),
            }),
        }
    }

    pub(crate) fn kind_name(&self) -> &'static str {
        match &*self.kind {
            WidgetKind::Identified { .. } => "Identified",
            WidgetKind::ActionScope(_) => "ActionScope",
            WidgetKind::Row(_) => "Row",
            WidgetKind::Column(_) => "Column",
            WidgetKind::Align(_) => "Align",
            WidgetKind::FocusScope(_) => "FocusScope",
            WidgetKind::SelectionRegion(_) => "SelectionRegion",
            WidgetKind::Clip(_) => "Clip",
            WidgetKind::Text(_) => "Text",
            WidgetKind::RichText(_) => "RichText",
            WidgetKind::Transform(_) => "Transform",
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(_) => "InteractiveViewer",
            WidgetKind::Button(_) => "Button",
            WidgetKind::Pressable(_) => "Pressable",
            WidgetKind::TextInput(_) => "TextInput",
            WidgetKind::Scroll(_) => "Scroll",
            WidgetKind::SemanticsRegion(_) => "SemanticsRegion",
            WidgetKind::Image(_) => "Image",
            WidgetKind::Video(_) => "Video",
            WidgetKind::ZStack(_) => "ZStack",
            WidgetKind::Overlay(_) => "Overlay",
            WidgetKind::Container(_) => "Container",
            WidgetKind::ContextMenuRegion(_) => "ContextMenuRegion",
            WidgetKind::GestureDetector(_) => "GestureDetector",
            WidgetKind::Grid(_) => "Grid",
            WidgetKind::GridItem(_) => "GridItem",
            WidgetKind::Responsive(_) => "Responsive",
            WidgetKind::Checkbox(_) => "Checkbox",
            WidgetKind::Switch(_) => "Switch",
            WidgetKind::Radio(_) => "Radio",
            WidgetKind::SafeArea(_) => "SafeArea",
            WidgetKind::Positioned(_) => "Positioned",
            WidgetKind::Spacer(_) => "Spacer",
            WidgetKind::Slider(_) => "Slider",
            WidgetKind::LazyColumn(_) => "LazyColumn",
            WidgetKind::Icon(_) => "Icon",
            WidgetKind::Composite(_) => "Composite",
            WidgetKind::Custom(_) => "Custom",
        }
    }

    fn kind_discriminator(&self) -> u32 {
        // These values are part of structural identity. Never renumber an
        // existing kind; append a new value even when variants move.
        match &*self.kind {
            WidgetKind::Identified { .. } => 1,
            WidgetKind::ActionScope(_) => 2,
            WidgetKind::Row(_) => 3,
            WidgetKind::Column(_) => 4,
            WidgetKind::Align(_) => 5,
            WidgetKind::FocusScope(_) => 6,
            WidgetKind::Clip(_) => 7,
            WidgetKind::Text(_) => 8,
            WidgetKind::RichText(_) => 9,
            WidgetKind::Transform(_) => 10,
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(_) => 11,
            WidgetKind::Button(_) => 12,
            WidgetKind::Pressable(_) => 13,
            WidgetKind::TextInput(_) => 14,
            WidgetKind::Scroll(_) => 15,
            WidgetKind::SemanticsRegion(_) => 16,
            WidgetKind::Image(_) => 17,
            WidgetKind::Video(_) => 18,
            WidgetKind::ZStack(_) => 19,
            WidgetKind::Overlay(_) => 20,
            WidgetKind::Container(_) => 21,
            WidgetKind::ContextMenuRegion(_) => 22,
            WidgetKind::GestureDetector(_) => 23,
            WidgetKind::Grid(_) => 24,
            WidgetKind::GridItem(_) => 25,
            WidgetKind::Responsive(_) => 26,
            WidgetKind::Checkbox(_) => 27,
            WidgetKind::Switch(_) => 28,
            WidgetKind::Radio(_) => 29,
            WidgetKind::SafeArea(_) => 30,
            WidgetKind::Positioned(_) => 31,
            WidgetKind::Spacer(_) => 32,
            WidgetKind::Slider(_) => 33,
            WidgetKind::LazyColumn(_) => 34,
            WidgetKind::Icon(_) => 35,
            WidgetKind::Composite(_) => 36,
            WidgetKind::Custom(_) => 37,
            WidgetKind::SelectionRegion(_) => 38,
        }
    }

    pub(crate) fn declared_id(&self) -> Option<WidgetId> {
        match &*self.kind {
            WidgetKind::Identified { id, .. } => Some(*id),
            WidgetKind::ActionScope(_) => None,
            WidgetKind::Custom(widget) => widget
                .lowerer
                .as_ref()
                .and_then(|lowerer| lowerer.widget_id()),
            WidgetKind::Row(widget) => widget.id,
            WidgetKind::Column(widget) => widget.id,
            WidgetKind::Align(widget) => widget.id,
            WidgetKind::FocusScope(widget) => widget.id,
            WidgetKind::SelectionRegion(widget) => widget.id,
            WidgetKind::Clip(widget) => widget.id,
            WidgetKind::Text(widget) => widget.id,
            WidgetKind::RichText(widget) => widget.id,
            WidgetKind::Transform(widget) => widget.id,
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(widget) => widget.id,
            WidgetKind::Button(widget) => widget.id,
            WidgetKind::Pressable(widget) => widget.id,
            WidgetKind::TextInput(widget) => widget.id,
            WidgetKind::Scroll(widget) => widget.id,
            WidgetKind::SemanticsRegion(widget) => widget.id,
            WidgetKind::Image(widget) => widget.id,
            WidgetKind::Video(widget) => widget.id,
            WidgetKind::ZStack(widget) => widget.id,
            WidgetKind::Overlay(widget) => widget.id,
            WidgetKind::Container(widget) => widget.id,
            WidgetKind::ContextMenuRegion(widget) => widget.id,
            WidgetKind::GestureDetector(widget) => widget.id,
            WidgetKind::Grid(widget) => widget.id,
            WidgetKind::GridItem(widget) => widget.id,
            WidgetKind::Responsive(widget) => widget.id,
            WidgetKind::Checkbox(widget) => widget.id,
            WidgetKind::Switch(widget) => widget.id,
            WidgetKind::Radio(widget) => widget.id,
            WidgetKind::SafeArea(widget) => widget.id,
            WidgetKind::Positioned(widget) => widget.id,
            WidgetKind::Spacer(widget) => widget.id,
            WidgetKind::Slider(widget) => widget.id,
            WidgetKind::LazyColumn(widget) => widget.id,
            WidgetKind::Icon(widget) => widget.id,
            WidgetKind::Composite(widget) => widget.id,
        }
    }

    pub(crate) fn resolve_identities(self, root: WidgetId) -> Self {
        self.resolve_identity(root)
    }

    fn resolve_identity(self, automatic_id: WidgetId) -> Self {
        let resolved = if self.declared_id().is_some() {
            self
        } else {
            self.with_id(automatic_id)
        };
        let parent = resolved.declared_id().unwrap_or(automatic_id);
        resolved.resolve_descendants(parent)
    }

    fn resolve_descendants(self, parent: WidgetId) -> Self {
        let child = |widget: Widget, slot: u32| {
            let id = WidgetId::derived(
                parent.as_u128(),
                &[Self::CHILD_ROLE, slot, widget.kind_discriminator()],
            );
            widget.resolve_identity(id)
        };
        let children = |widgets: Vec<Widget>, first_slot: u32| {
            widgets
                .into_iter()
                .enumerate()
                .map(|(index, widget)| child(widget, first_slot + index as u32))
                .collect()
        };

        let kind = match *self.kind {
            WidgetKind::Identified {
                id,
                child: identified_child,
            } => WidgetKind::Identified {
                id,
                // The structural wrapper is the logical identity for widget
                // kinds that cannot store an id directly (ActionScope and
                // Custom). Re-resolving that child would create wrappers
                // recursively; only its descendants need identities here.
                child: identified_child.resolve_descendants(id),
            },
            WidgetKind::ActionScope(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::ActionScope(widget)
            }
            WidgetKind::Row(mut widget) => {
                widget.children = children(widget.children, 0);
                WidgetKind::Row(widget)
            }
            WidgetKind::Column(mut widget) => {
                widget.children = children(widget.children, 0);
                WidgetKind::Column(widget)
            }
            WidgetKind::Align(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::Align(widget)
            }
            WidgetKind::FocusScope(mut widget) => {
                widget.children = children(widget.children, 0);
                WidgetKind::FocusScope(widget)
            }
            WidgetKind::SelectionRegion(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::SelectionRegion(widget)
            }
            WidgetKind::Clip(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::Clip(widget)
            }
            WidgetKind::Text(widget) => WidgetKind::Text(widget),
            WidgetKind::RichText(mut widget) => {
                for (index, inline) in widget.inline_widgets.iter_mut().enumerate() {
                    inline.widget = child(inline.widget.clone(), index as u32);
                }
                WidgetKind::RichText(widget)
            }
            WidgetKind::Transform(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::Transform(widget)
            }
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::InteractiveViewer(widget)
            }
            WidgetKind::Button(mut widget) => {
                widget.child = widget.child.map(|value| child(value, 0));
                WidgetKind::Button(widget)
            }
            WidgetKind::Pressable(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::Pressable(widget)
            }
            WidgetKind::TextInput(mut widget) => {
                widget.prefix = widget.prefix.map(|value| child(value, 0));
                widget.suffix = widget.suffix.map(|value| child(value, 1));
                WidgetKind::TextInput(widget)
            }
            WidgetKind::Scroll(mut widget) => {
                widget.child = widget.child.map(|value| child(value, 0));
                WidgetKind::Scroll(widget)
            }
            WidgetKind::SemanticsRegion(mut widget) => {
                widget.child = widget.child.map(|value| child(value, 0));
                WidgetKind::SemanticsRegion(widget)
            }
            WidgetKind::Image(widget) => WidgetKind::Image(widget),
            WidgetKind::Video(widget) => WidgetKind::Video(widget),
            WidgetKind::ZStack(mut widget) => {
                widget.children = children(widget.children, 0);
                WidgetKind::ZStack(widget)
            }
            WidgetKind::Overlay(mut widget) => {
                widget.content = child(widget.content, 0);
                widget.overlay = child(widget.overlay, 1);
                WidgetKind::Overlay(widget)
            }
            WidgetKind::Container(mut widget) => {
                widget.child = widget.child.map(|value| child(value, 0));
                WidgetKind::Container(widget)
            }
            WidgetKind::ContextMenuRegion(mut widget) => {
                widget.child = child(widget.child, 0);
                for (index, entry) in widget.menu.items.iter_mut().enumerate() {
                    if let ContextMenuEntry::Item(item) = entry {
                        item.child = child(item.child.clone(), 1 + index as u32);
                    }
                }
                WidgetKind::ContextMenuRegion(widget)
            }
            WidgetKind::GestureDetector(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::GestureDetector(widget)
            }
            WidgetKind::Grid(mut widget) => {
                widget.children = children(widget.children, 0);
                WidgetKind::Grid(widget)
            }
            WidgetKind::GridItem(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::GridItem(widget)
            }
            WidgetKind::Responsive(mut widget) => {
                for (index, case) in widget.cases.iter_mut().enumerate() {
                    case.child = child(case.child.clone(), index as u32);
                }
                widget.fallback = child(widget.fallback, u32::MAX);
                WidgetKind::Responsive(widget)
            }
            WidgetKind::Checkbox(widget) => WidgetKind::Checkbox(widget),
            WidgetKind::Switch(widget) => WidgetKind::Switch(widget),
            WidgetKind::Radio(widget) => WidgetKind::Radio(widget),
            WidgetKind::SafeArea(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::SafeArea(widget)
            }
            WidgetKind::Positioned(mut widget) => {
                widget.child = widget.child.map(|value| child(value, 0));
                WidgetKind::Positioned(widget)
            }
            WidgetKind::Spacer(widget) => WidgetKind::Spacer(widget),
            WidgetKind::Slider(widget) => WidgetKind::Slider(widget),
            WidgetKind::LazyColumn(mut widget) => {
                widget.children = children(widget.children, 0);
                WidgetKind::LazyColumn(widget)
            }
            WidgetKind::Icon(widget) => WidgetKind::Icon(widget),
            WidgetKind::Composite(mut widget) => {
                widget.child = child(widget.child, 0);
                WidgetKind::Composite(widget)
            }
            WidgetKind::Custom(widget) => WidgetKind::Custom(widget),
        };

        Self {
            kind: Box::new(kind),
        }
    }

    pub(crate) fn as_row(&self) -> Option<&Row> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_row(),
            WidgetKind::Row(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_column(&self) -> Option<&Column> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_column(),
            WidgetKind::Column(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_container(&self) -> Option<&Container> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_container(),
            WidgetKind::Container(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_scroll(&self) -> Option<&Scroll> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_scroll(),
            WidgetKind::Scroll(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_rich_text(&self) -> Option<&RichText> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_rich_text(),
            WidgetKind::RichText(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_text(&self) -> Option<&Text> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_text(),
            WidgetKind::Text(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_text_input(&self) -> Option<&TextInput> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_text_input(),
            WidgetKind::TextInput(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_button(&self) -> Option<&Button> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_button(),
            WidgetKind::Button(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_gesture_detector(&self) -> Option<&GestureDetector> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_gesture_detector(),
            WidgetKind::GestureDetector(widget) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn as_zstack(&self) -> Option<&ZStack> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_zstack(),
            WidgetKind::ZStack(widget) => Some(widget),
            _ => None,
        }
    }

    #[cfg(feature = "interactive-canvas")]
    pub(crate) fn as_interactive_viewer(&self) -> Option<&InteractiveViewer> {
        match &*self.kind {
            WidgetKind::Identified { child, .. } => child.as_interactive_viewer(),
            WidgetKind::InteractiveViewer(widget) => Some(widget),
            _ => None,
        }
    }
}

/// Overrides Fission's automatic structural identity for a widget.
///
/// Explicit IDs are normally only needed for logical items in dynamic
/// collections or for code that must address a particular widget. An explicit
/// ID also scopes all automatically identified descendants, so a stateful
/// subtree follows its logical item when reordered.
pub trait WidgetIdExt: Into<Widget> + Sized {
    fn id<I>(self, id: I) -> Widget
    where
        I: Into<WidgetId>,
    {
        let id = id.into();
        crate::build::with_widget_id(id, || {
            let widget: Widget = self.into();
            widget.with_id(id)
        })
    }
}

impl<T> WidgetIdExt for T where T: Into<Widget> {}

impl Widget {
    pub(crate) fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        match &*self.kind {
            WidgetKind::Identified { id, child } => {
                cx.push_scope(*id);
                let child_id = child.lower(cx);
                cx.pop_scope();
                let mut builder = crate::lowering::InternalIrBuilder::new(
                    (*id).into(),
                    Op::Structural(StructuralOp::Group {
                        stable_hash: id.as_u128() as u64,
                    }),
                );
                builder.add_child(child_id);
                builder.build(cx)
            }
            WidgetKind::ActionScope(w) => w.lower(cx),
            WidgetKind::Row(w) => w.lower(cx),
            WidgetKind::Column(w) => w.lower(cx),
            WidgetKind::Align(w) => w.lower(cx),
            WidgetKind::FocusScope(w) => w.lower(cx),
            WidgetKind::SelectionRegion(w) => w.lower(cx),
            WidgetKind::Clip(w) => w.lower(cx),
            WidgetKind::Text(w) => w.lower(cx),
            WidgetKind::RichText(w) => w.lower(cx),
            WidgetKind::Transform(w) => w.lower(cx),
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(w) => w.lower(cx),
            WidgetKind::Button(w) => w.lower(cx),
            WidgetKind::Pressable(w) => w.lower(cx),
            WidgetKind::TextInput(w) => w.lower(cx),
            WidgetKind::Scroll(w) => w.lower(cx),
            WidgetKind::SemanticsRegion(w) => w.lower(cx),
            WidgetKind::Image(w) => w.lower(cx),
            WidgetKind::Video(w) => w.lower(cx),
            WidgetKind::ZStack(w) => w.lower(cx),
            WidgetKind::Overlay(w) => w.lower(cx),
            WidgetKind::Container(w) => w.lower(cx),
            WidgetKind::ContextMenuRegion(w) => w.lower(cx),
            WidgetKind::GestureDetector(w) => w.lower(cx),
            WidgetKind::Grid(w) => w.lower(cx),
            WidgetKind::GridItem(w) => w.lower(cx),
            WidgetKind::Responsive(w) => w.lower(cx),
            WidgetKind::Checkbox(w) => w.lower(cx),
            WidgetKind::Switch(w) => w.lower(cx),
            WidgetKind::Radio(w) => w.lower(cx),
            WidgetKind::SafeArea(w) => w.lower(cx),
            WidgetKind::Positioned(w) => w.lower(cx),
            WidgetKind::Spacer(w) => w.lower(cx),
            WidgetKind::Slider(w) => w.lower(cx),
            WidgetKind::LazyColumn(w) => w.lower(cx),
            WidgetKind::Icon(w) => w.lower(cx),
            WidgetKind::Composite(w) => w.lower(cx),
            WidgetKind::Custom(w) => {
                let lowerer = w
                    .lowerer
                    .as_ref()
                    .expect("CustomWidget lowerer must be set");
                let wrapper = lowerer.widget_id().unwrap_or_else(|| cx.next_node_id());
                cx.push_scope(wrapper);
                let child_id = lowerer.lower_dyn(cx);
                cx.pop_scope();
                let mut builder = crate::lowering::InternalIrBuilder::new(
                    wrapper,
                    Op::Structural(StructuralOp::Group {
                        stable_hash: lowerer.stable_key(),
                    }),
                );
                builder.add_child(child_id);
                let node_id = builder.build(cx);

                // If the custom node carries a render object, store it in the
                // IR so that hit-testing and event handling can find it later.
                // We wrap the `Arc<dyn CustomRenderObject>` in a `RenderObjectHolder`
                // so it can be stored as `Arc<dyn Any + Send + Sync>` in the
                // dependency-free IR crate and downcast back later.
                if let Some(render_obj) = &w.render_object {
                    let holder = crate::ui::custom_render::RenderObjectHolder(render_obj.clone());
                    let erased: fission_ir::AnyRenderObject = Arc::new(holder);
                    cx.ir.custom_render_objects.insert(node_id, erased);
                }

                node_id
            }
        }
    }
}

impl From<Row> for Widget {
    fn from(w: Row) -> Self {
        Self {
            kind: Box::new(WidgetKind::Row(w)),
        }
    }
}
impl From<ActionScope> for Widget {
    fn from(w: ActionScope) -> Self {
        Self {
            kind: Box::new(WidgetKind::ActionScope(w)),
        }
    }
}
impl From<Column> for Widget {
    fn from(w: Column) -> Self {
        Self {
            kind: Box::new(WidgetKind::Column(w)),
        }
    }
}
impl From<Align> for Widget {
    fn from(w: Align) -> Self {
        Self {
            kind: Box::new(WidgetKind::Align(w)),
        }
    }
}
impl From<FocusScope> for Widget {
    fn from(w: FocusScope) -> Self {
        Self {
            kind: Box::new(WidgetKind::FocusScope(w)),
        }
    }
}
impl From<SelectionRegion> for Widget {
    fn from(w: SelectionRegion) -> Self {
        Self {
            kind: Box::new(WidgetKind::SelectionRegion(w)),
        }
    }
}
impl From<Clip> for Widget {
    fn from(w: Clip) -> Self {
        Self {
            kind: Box::new(WidgetKind::Clip(w)),
        }
    }
}
impl From<Text> for Widget {
    fn from(w: Text) -> Self {
        Self {
            kind: Box::new(WidgetKind::Text(w)),
        }
    }
}
impl From<RichText> for Widget {
    fn from(w: RichText) -> Self {
        Self {
            kind: Box::new(WidgetKind::RichText(w)),
        }
    }
}
impl From<Transform> for Widget {
    fn from(w: Transform) -> Self {
        Self {
            kind: Box::new(WidgetKind::Transform(w)),
        }
    }
}
#[cfg(feature = "interactive-canvas")]
impl From<InteractiveViewer> for Widget {
    fn from(w: InteractiveViewer) -> Self {
        Self {
            kind: Box::new(WidgetKind::InteractiveViewer(w)),
        }
    }
}
impl From<Button> for Widget {
    fn from(mut w: Button) -> Self {
        if let Some(motion) = w.motion.take() {
            let button_id = crate::build::current_widget_id()
                .or(w.id)
                .unwrap_or_else(|| WidgetId::explicit("fission.core.button.motion"));
            w.id = Some(button_id);
            let motion_id = WidgetId::derived(button_id.as_u128(), &[0xB0770]);
            let tracks = motion.interaction_tracks(button_id);
            let ripple = motion.ripple();
            let base = Self {
                kind: Box::new(WidgetKind::Button(w)),
            };
            let with_motion: Widget = if tracks.is_empty() {
                base
            } else {
                crate::motion::Motion {
                    id: motion_id,
                    tracks,
                    child: base,
                    ..Default::default()
                }
                .into()
            };
            return if let Some(effect) = ripple {
                crate::motion::RippleLayer {
                    id: WidgetId::derived(button_id.as_u128(), &[0xA11E]),
                    effect,
                    child: with_motion,
                }
                .into()
            } else {
                with_motion
            };
        }
        Self {
            kind: Box::new(WidgetKind::Button(w)),
        }
    }
}
impl From<TextInput> for Widget {
    fn from(w: TextInput) -> Self {
        Self {
            kind: Box::new(WidgetKind::TextInput(w)),
        }
    }
}
impl From<Scroll> for Widget {
    fn from(w: Scroll) -> Self {
        Self {
            kind: Box::new(WidgetKind::Scroll(w)),
        }
    }
}
impl From<SemanticsRegion> for Widget {
    fn from(w: SemanticsRegion) -> Self {
        Self {
            kind: Box::new(WidgetKind::SemanticsRegion(w)),
        }
    }
}
impl From<Image> for Widget {
    fn from(w: Image) -> Self {
        Self {
            kind: Box::new(WidgetKind::Image(w)),
        }
    }
}
impl From<Video> for Widget {
    fn from(w: Video) -> Self {
        let node_id = crate::build::current_widget_id()
            .or(w.id)
            .unwrap_or_else(|| fission_ir::WidgetId::explicit(&w.source.key()));
        crate::build::try_register_video(crate::registry::VideoRegistration {
            node_id,
            source: w.source.as_str().to_string(),
            autoplay: w.autoplay,
            loop_playback: w.loop_playback,
            audio: w.audio.clone(),
        });
        Self {
            kind: Box::new(WidgetKind::Video(w)),
        }
    }
}
impl From<ZStack> for Widget {
    fn from(w: ZStack) -> Self {
        Self {
            kind: Box::new(WidgetKind::ZStack(w)),
        }
    }
}
impl From<Overlay> for Widget {
    fn from(w: Overlay) -> Self {
        Self {
            kind: Box::new(WidgetKind::Overlay(w)),
        }
    }
}
impl From<ContextMenuRegion> for Widget {
    fn from(w: ContextMenuRegion) -> Self {
        Self {
            kind: Box::new(WidgetKind::ContextMenuRegion(w)),
        }
    }
}

impl From<Container> for Widget {
    fn from(w: Container) -> Self {
        Self {
            kind: Box::new(WidgetKind::Container(w)),
        }
    }
}
impl From<GestureDetector> for Widget {
    fn from(w: GestureDetector) -> Self {
        Self {
            kind: Box::new(WidgetKind::GestureDetector(w)),
        }
    }
}
impl From<Grid> for Widget {
    fn from(w: Grid) -> Self {
        Self {
            kind: Box::new(WidgetKind::Grid(w)),
        }
    }
}
impl From<GridItem> for Widget {
    fn from(w: GridItem) -> Self {
        Self {
            kind: Box::new(WidgetKind::GridItem(w)),
        }
    }
}
impl From<Responsive> for Widget {
    fn from(w: Responsive) -> Self {
        Self {
            kind: Box::new(WidgetKind::Responsive(w)),
        }
    }
}
impl From<Checkbox> for Widget {
    fn from(w: Checkbox) -> Self {
        Self {
            kind: Box::new(WidgetKind::Checkbox(w)),
        }
    }
}
impl From<Switch> for Widget {
    fn from(w: Switch) -> Self {
        Self {
            kind: Box::new(WidgetKind::Switch(w)),
        }
    }
}
impl From<Radio> for Widget {
    fn from(w: Radio) -> Self {
        Self {
            kind: Box::new(WidgetKind::Radio(w)),
        }
    }
}
impl From<SafeArea> for Widget {
    fn from(w: SafeArea) -> Self {
        Self {
            kind: Box::new(WidgetKind::SafeArea(w)),
        }
    }
}
impl From<Composite> for Widget {
    fn from(w: Composite) -> Self {
        Self {
            kind: Box::new(WidgetKind::Composite(w)),
        }
    }
}
impl From<Positioned> for Widget {
    fn from(w: Positioned) -> Self {
        Self {
            kind: Box::new(WidgetKind::Positioned(w)),
        }
    }
}
impl From<Spacer> for Widget {
    fn from(w: Spacer) -> Self {
        Self {
            kind: Box::new(WidgetKind::Spacer(w)),
        }
    }
}
impl From<Slider> for Widget {
    fn from(w: Slider) -> Self {
        Self {
            kind: Box::new(WidgetKind::Slider(w)),
        }
    }
}
impl From<LazyColumn> for Widget {
    fn from(w: LazyColumn) -> Self {
        Self {
            kind: Box::new(WidgetKind::LazyColumn(w)),
        }
    }
}
impl From<Icon> for Widget {
    fn from(w: Icon) -> Self {
        Self {
            kind: Box::new(WidgetKind::Icon(w)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InternalRenderNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn InternalLowerer>>,
    /// Optional render object that participates in hit-testing, event handling,
    /// and painting.  When `None`, the node behaves exactly as before (lowering
    /// only via `InternalLowerer`).
    #[serde(skip)]
    pub render_object: Option<Arc<dyn CustomRenderObject>>,
}

pub type CustomWidget = InternalRenderNode;

impl From<CustomWidget> for Widget {
    fn from(node: CustomWidget) -> Self {
        Widget::custom(node)
    }
}

#[cfg(test)]
mod visitor_tests {
    use super::*;

    #[test]
    fn visitor_walks_nested_widgets_and_can_stop() {
        let root: Widget = Column {
            children: vec![Text::new("first").into(), Text::new("second").into()],
            ..Default::default()
        }
        .into();
        let mut visited = 0;
        let result = root.visit(&mut |widget| {
            visited += 1;
            if matches!(widget.kind(), WidgetKind::Text(_)) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert!(matches!(result, ControlFlow::Break(())));
        assert_eq!(visited, 2);
    }
}
