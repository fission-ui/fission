use super::custom_render::CustomRenderObject;
use super::traits::{Lower, LowerWidget};
#[cfg(feature = "interactive-canvas")]
use super::widgets::InteractiveViewer;
use super::widgets::{
    ActionScope, Align, Button, Checkbox, Clip, Column, Composite, Container, ContextMenuEntry,
    ContextMenuRegion, FocusScope, GestureDetector, Grid, GridItem, Icon, Image, LazyColumn,
    Overlay, Positioned, Pressable, Radio, Responsive, RichText, Row, SafeArea, Scroll,
    SelectionRegion, SemanticsRegion, Slider, Spacer, Switch, Text, TextInput, Transform, Video,
    ZStack,
};
use crate::lowering::{FormFieldContext, LoweringContext};
use fission_ir::{CoreIR, Op, Role, StructuralOp, TextFieldValidationState, WidgetId};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::ops::ControlFlow;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Widget {
    kind: Box<WidgetKind>,
    /// Relationship metadata applied to one unambiguous form-control
    /// semantics node after this retained subtree is lowered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    form_field_relationships: Option<Box<FormFieldRelationships>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FormFieldRelationships {
    labelled_by: Vec<WidgetId>,
    described_by: Vec<WidgetId>,
    required: bool,
    invalid_message: Option<String>,
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

fn extend_unique(target: &mut Vec<WidgetId>, values: impl IntoIterator<Item = WidgetId>) {
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

fn is_form_control_role(role: Role) -> bool {
    matches!(
        role,
        Role::TextInput
            | Role::Input
            | Role::ComboBox
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Slider
    )
}

fn retained_form_control_count(widget: &Widget) -> usize {
    let mut count = 0usize;
    let _ = widget.visit(&mut |candidate| {
        let is_control = match candidate.kind() {
            WidgetKind::TextInput(_)
            | WidgetKind::Checkbox(_)
            | WidgetKind::Radio(_)
            | WidgetKind::Switch(_)
            | WidgetKind::Slider(_) => true,
            WidgetKind::Button(button) => button
                .semantics
                .as_ref()
                .is_some_and(|semantics| is_form_control_role(semantics.role)),
            _ => false,
        };
        if is_control {
            count += 1;
        }
        if count > 1 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    count
}

fn form_control_semantics_in_subtree(ir: &CoreIR, root: WidgetId) -> Vec<WidgetId> {
    let mut controls = Vec::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let Some(node) = ir.nodes.get(&id) else {
            continue;
        };
        if matches!(&node.op, Op::Semantics(semantics) if is_form_control_role(semantics.role)) {
            controls.push(id);
            if controls.len() > 1 {
                break;
            }
        }
        pending.extend(node.children.iter().rev().copied());
    }
    controls
}

fn refresh_subtree_hashes(ir: &mut CoreIR, root: WidgetId) -> u64 {
    let children = ir
        .nodes
        .get(&root)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    for child in &children {
        refresh_subtree_hashes(ir, *child);
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Some(node) = ir.nodes.get(&root) {
        node.op.hash(&mut hasher);
        node.composite.hash(&mut hasher);
        for child in &children {
            if let Some(child_node) = ir.nodes.get(child) {
                child_node.hash.hash(&mut hasher);
            }
            child.hash(&mut hasher);
        }
    }
    let hash = hasher.finish();
    if let Some(node) = ir.nodes.get_mut(&root) {
        node.hash = hash;
    }
    hash
}

fn apply_form_field_relationships(
    ir: &mut CoreIR,
    root: WidgetId,
    relationships: &FormFieldRelationships,
) {
    let controls = form_control_semantics_in_subtree(ir, root);
    let [control_id] = controls.as_slice() else {
        return;
    };
    let Some(node) = ir.nodes.get_mut(control_id) else {
        return;
    };
    let Op::Semantics(semantics) = &mut node.op else {
        return;
    };

    extend_unique(
        &mut semantics.labelled_by,
        relationships.labelled_by.iter().copied(),
    );
    extend_unique(
        &mut semantics.described_by,
        relationships.described_by.iter().copied(),
    );
    semantics.required |= relationships.required;
    if let Some(message) = &relationships.invalid_message {
        semantics.validation_state = TextFieldValidationState::Invalid;
        semantics.validation_message = Some(message.clone());
    }

    refresh_subtree_hashes(ir, root);
}

impl Widget {
    const CHILD_ROLE: u32 = 0xF155_2000;

    fn from_kind(kind: WidgetKind) -> Self {
        Self {
            kind: Box::new(kind),
            form_field_relationships: None,
        }
    }

    /// Associates external form-field labels and messages with this subtree's
    /// actual control.
    ///
    /// This operation is deliberately narrower than a generic semantics-tree
    /// mutator. During lowering it augments the sole semantic descendant whose
    /// role is a built-in form control. If the subtree contains no eligible
    /// control, or contains more than one, it is left unchanged rather than
    /// guessing which control owns the relationships.
    ///
    /// Existing relationships, identity, actions, value, selection, and text
    /// editing configuration are preserved. `required` is additive. An invalid
    /// message, when supplied, becomes the field's authoritative invalid state.
    #[doc(hidden)]
    pub fn with_form_field_relationships(
        mut self,
        labelled_by: Vec<WidgetId>,
        described_by: Vec<WidgetId>,
        required: bool,
        invalid_message: Option<String>,
    ) -> Self {
        let relationships = self.form_field_relationships.get_or_insert_with(|| {
            Box::new(FormFieldRelationships {
                labelled_by: Vec::new(),
                described_by: Vec::new(),
                required: false,
                invalid_message: None,
            })
        });
        extend_unique(&mut relationships.labelled_by, labelled_by);
        extend_unique(&mut relationships.described_by, described_by);
        relationships.required |= required;
        if invalid_message.is_some() {
            relationships.invalid_message = invalid_message;
        }
        self
    }

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

    pub(crate) fn with_id(mut self, id: WidgetId) -> Self {
        self.set_id_in_place(id);
        self
    }

    /// Applies an identity without moving the concrete widget value through a
    /// large `WidgetKind` match frame. This matters on Wasm, where deeply
    /// composed production trees have a materially smaller native stack.
    fn set_id_in_place(&mut self, id: WidgetId) {
        if matches!(
            self.kind.as_ref(),
            WidgetKind::ActionScope(_) | WidgetKind::Custom(_)
        ) {
            let kind = std::mem::replace(
                &mut self.kind,
                Box::new(WidgetKind::Spacer(Spacer::default())),
            );
            self.kind = Box::new(WidgetKind::Identified {
                id,
                child: Self {
                    kind,
                    form_field_relationships: None,
                },
            });
            return;
        }

        match self.kind.as_mut() {
            WidgetKind::Identified {
                id: existing_id, ..
            } => *existing_id = id,
            WidgetKind::Row(widget) => widget.id = Some(id),
            WidgetKind::Column(widget) => widget.id = Some(id),
            WidgetKind::Align(widget) => widget.id = Some(id),
            WidgetKind::FocusScope(widget) => widget.id = Some(id),
            WidgetKind::SelectionRegion(widget) => widget.id = Some(id),
            WidgetKind::Clip(widget) => widget.id = Some(id),
            WidgetKind::Text(widget) => widget.id = Some(id),
            WidgetKind::RichText(widget) => widget.id = Some(id),
            WidgetKind::Transform(widget) => widget.id = Some(id),
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(widget) => widget.id = Some(id),
            WidgetKind::Button(widget) => {
                if let Some(previous_id) = widget.id {
                    crate::build::try_remove_motion_declarations(previous_id);
                }
                widget.id = Some(id);
                widget.register_motion_declarations(id);
            }
            WidgetKind::Pressable(widget) => widget.id = Some(id),
            WidgetKind::TextInput(widget) => widget.id = Some(id),
            WidgetKind::Scroll(widget) => widget.id = Some(id),
            WidgetKind::SemanticsRegion(widget) => widget.id = Some(id),
            WidgetKind::Image(widget) => widget.id = Some(id),
            WidgetKind::Video(widget) => widget.id = Some(id),
            WidgetKind::ZStack(widget) => widget.id = Some(id),
            WidgetKind::Overlay(widget) => widget.id = Some(id),
            WidgetKind::Container(widget) => widget.id = Some(id),
            WidgetKind::ContextMenuRegion(widget) => widget.id = Some(id),
            WidgetKind::GestureDetector(widget) => widget.id = Some(id),
            WidgetKind::Grid(widget) => widget.id = Some(id),
            WidgetKind::GridItem(widget) => widget.id = Some(id),
            WidgetKind::Responsive(widget) => widget.id = Some(id),
            WidgetKind::Checkbox(widget) => widget.id = Some(id),
            WidgetKind::Switch(widget) => widget.id = Some(id),
            WidgetKind::Radio(widget) => widget.id = Some(id),
            WidgetKind::SafeArea(widget) => widget.id = Some(id),
            WidgetKind::Positioned(widget) => widget.id = Some(id),
            WidgetKind::Spacer(widget) => widget.id = Some(id),
            WidgetKind::Slider(widget) => widget.id = Some(id),
            WidgetKind::LazyColumn(widget) => widget.id = Some(id),
            WidgetKind::Icon(widget) => widget.id = Some(id),
            WidgetKind::Composite(widget) => widget.id = Some(id),
            WidgetKind::ActionScope(_) | WidgetKind::Custom(_) => {
                unreachable!("scope-only kinds are wrapped before assigning identity")
            }
        }
    }

    pub fn id<I>(self, id: I) -> Self
    where
        I: Into<WidgetId>,
    {
        self.with_id(id.into())
    }

    pub(crate) fn custom(node: InternalRenderNode) -> Self {
        Self::from_kind(WidgetKind::Custom(node))
    }

    pub(crate) fn from_pressable_raw(pressable: Pressable) -> Self {
        Self::from_kind(WidgetKind::Pressable(pressable))
    }

    pub(crate) fn into_text(self) -> Result<Text, Self> {
        let Self {
            kind,
            form_field_relationships,
        } = self;
        match (*kind, form_field_relationships) {
            (WidgetKind::Text(text), None) => Ok(text),
            (kind, form_field_relationships) => Err(Self {
                kind: Box::new(kind),
                form_field_relationships,
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
        let mut resolved = self;
        resolved.resolve_identity_in_place(root);
        resolved
    }

    fn resolve_identity_in_place(&mut self, automatic_id: WidgetId) {
        if self.declared_id().is_none() {
            self.set_id_in_place(automatic_id);
        }
        let parent = self.declared_id().unwrap_or(automatic_id);
        self.resolve_descendants_in_place(parent);
    }

    fn resolve_child_identity(parent: WidgetId, slot: u32, widget: &mut Widget) {
        let id = WidgetId::derived(
            parent.as_u128(),
            &[Self::CHILD_ROLE, slot, widget.kind_discriminator()],
        );
        widget.resolve_identity_in_place(id);
    }

    fn resolve_children_identities(parent: WidgetId, first_slot: u32, widgets: &mut [Widget]) {
        for (index, widget) in widgets.iter_mut().enumerate() {
            Self::resolve_child_identity(parent, first_slot + index as u32, widget);
        }
    }

    fn resolve_descendants_in_place(&mut self, parent: WidgetId) {
        match self.kind.as_mut() {
            WidgetKind::Identified {
                id,
                child: identified_child,
            } => {
                // The structural wrapper is the logical identity for widget
                // kinds that cannot store an id directly (ActionScope and
                // Custom). Re-resolving that child would create wrappers
                // recursively; only its descendants need identities here.
                identified_child.resolve_descendants_in_place(*id);
            }
            WidgetKind::ActionScope(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Row(widget) => {
                Self::resolve_children_identities(parent, 0, &mut widget.children);
            }
            WidgetKind::Column(widget) => {
                Self::resolve_children_identities(parent, 0, &mut widget.children);
            }
            WidgetKind::Align(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::FocusScope(widget) => {
                Self::resolve_children_identities(parent, 0, &mut widget.children);
            }
            WidgetKind::SelectionRegion(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Clip(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Text(_) => {}
            WidgetKind::RichText(widget) => {
                for (index, inline) in widget.inline_widgets.iter_mut().enumerate() {
                    Self::resolve_child_identity(parent, index as u32, &mut inline.widget);
                }
            }
            WidgetKind::Transform(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            #[cfg(feature = "interactive-canvas")]
            WidgetKind::InteractiveViewer(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Button(widget) => {
                if let Some(child) = &mut widget.child {
                    Self::resolve_child_identity(parent, 0, child);
                }
            }
            WidgetKind::Pressable(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::TextInput(widget) => {
                if let Some(prefix) = &mut widget.prefix {
                    Self::resolve_child_identity(parent, 0, prefix);
                }
                if let Some(suffix) = &mut widget.suffix {
                    Self::resolve_child_identity(parent, 1, suffix);
                }
            }
            WidgetKind::Scroll(widget) => {
                if let Some(child) = &mut widget.child {
                    Self::resolve_child_identity(parent, 0, child);
                }
            }
            WidgetKind::SemanticsRegion(widget) => {
                if let Some(child) = &mut widget.child {
                    Self::resolve_child_identity(parent, 0, child);
                }
            }
            WidgetKind::Image(_) | WidgetKind::Video(_) => {}
            WidgetKind::ZStack(widget) => {
                Self::resolve_children_identities(parent, 0, &mut widget.children);
            }
            WidgetKind::Overlay(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.content);
                Self::resolve_child_identity(parent, 1, &mut widget.overlay);
            }
            WidgetKind::Container(widget) => {
                if let Some(child) = &mut widget.child {
                    Self::resolve_child_identity(parent, 0, child);
                }
            }
            WidgetKind::ContextMenuRegion(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
                for (index, entry) in widget.menu.items.iter_mut().enumerate() {
                    if let ContextMenuEntry::Item(item) = entry {
                        Self::resolve_child_identity(parent, 1 + index as u32, &mut item.child);
                    }
                }
            }
            WidgetKind::GestureDetector(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Grid(widget) => {
                Self::resolve_children_identities(parent, 0, &mut widget.children);
            }
            WidgetKind::GridItem(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Responsive(widget) => {
                for (index, case) in widget.cases.iter_mut().enumerate() {
                    Self::resolve_child_identity(parent, index as u32, &mut case.child);
                }
                Self::resolve_child_identity(parent, u32::MAX, &mut widget.fallback);
            }
            WidgetKind::Checkbox(_) | WidgetKind::Switch(_) | WidgetKind::Radio(_) => {}
            WidgetKind::SafeArea(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Positioned(widget) => {
                if let Some(child) = &mut widget.child {
                    Self::resolve_child_identity(parent, 0, child);
                }
            }
            WidgetKind::Spacer(_) | WidgetKind::Slider(_) => {}
            WidgetKind::LazyColumn(widget) => {
                Self::resolve_children_identities(parent, 0, &mut widget.children);
            }
            WidgetKind::Icon(_) => {}
            WidgetKind::Composite(widget) => {
                Self::resolve_child_identity(parent, 0, &mut widget.child);
            }
            WidgetKind::Custom(_) => {}
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
    pub(crate) fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let has_form_field_context =
            self.form_field_relationships.is_some() && retained_form_control_count(self) == 1;
        if let Some(relationships) = self
            .form_field_relationships
            .as_ref()
            .filter(|_| has_form_field_context)
        {
            cx.push_form_field_context(FormFieldContext {
                required: relationships.required,
                invalid_message: relationships.invalid_message.clone(),
            });
        }
        let root = match &*self.kind {
            WidgetKind::Identified { id, child } => {
                cx.push_scope(*id);
                let child_id = child.lower(cx);
                cx.pop_scope();
                let mut builder = crate::lowering::IrBuilder::new(
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
                let mut builder = crate::lowering::IrBuilder::new(
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
        };
        if has_form_field_context {
            cx.pop_form_field_context();
        }
        if let Some(relationships) = &self.form_field_relationships {
            apply_form_field_relationships(&mut cx.ir, root, relationships);
        }
        root
    }
}

impl From<Row> for Widget {
    fn from(w: Row) -> Self {
        Self::from_kind(WidgetKind::Row(w))
    }
}
impl From<ActionScope> for Widget {
    fn from(w: ActionScope) -> Self {
        Self::from_kind(WidgetKind::ActionScope(w))
    }
}
impl From<Column> for Widget {
    fn from(w: Column) -> Self {
        Self::from_kind(WidgetKind::Column(w))
    }
}
impl From<Align> for Widget {
    fn from(w: Align) -> Self {
        Self::from_kind(WidgetKind::Align(w))
    }
}
impl From<FocusScope> for Widget {
    fn from(w: FocusScope) -> Self {
        Self::from_kind(WidgetKind::FocusScope(w))
    }
}
impl From<SelectionRegion> for Widget {
    fn from(w: SelectionRegion) -> Self {
        Self::from_kind(WidgetKind::SelectionRegion(w))
    }
}
impl From<Clip> for Widget {
    fn from(w: Clip) -> Self {
        Self::from_kind(WidgetKind::Clip(w))
    }
}
impl From<Text> for Widget {
    fn from(w: Text) -> Self {
        Self::from_kind(WidgetKind::Text(w))
    }
}
impl From<RichText> for Widget {
    fn from(w: RichText) -> Self {
        Self::from_kind(WidgetKind::RichText(w))
    }
}
impl From<Transform> for Widget {
    fn from(w: Transform) -> Self {
        Self::from_kind(WidgetKind::Transform(w))
    }
}
#[cfg(feature = "interactive-canvas")]
impl From<InteractiveViewer> for Widget {
    fn from(w: InteractiveViewer) -> Self {
        Self::from_kind(WidgetKind::InteractiveViewer(w))
    }
}
impl From<Button> for Widget {
    fn from(mut w: Button) -> Self {
        let current_widget_id = crate::build::current_widget_id();
        let inherited_root_id = crate::build::current_identity()
            .filter(|identity| Some(*identity) == current_widget_id);
        let button_id =
            w.id.or(inherited_root_id)
                .or_else(|| crate::build::next_implicit_widget_id(Button::MOTION_SALT));
        let Some(button_id) = button_id else {
            // Implicit identities and motion declarations are build-scoped.
            return Self::from_kind(WidgetKind::Button(w));
        };
        w.id = Some(button_id);
        w.register_motion_declarations(button_id);
        Self::from_kind(WidgetKind::Button(w))
    }
}
impl From<TextInput> for Widget {
    fn from(w: TextInput) -> Self {
        Self::from_kind(WidgetKind::TextInput(w))
    }
}
impl From<Scroll> for Widget {
    fn from(w: Scroll) -> Self {
        Self::from_kind(WidgetKind::Scroll(w))
    }
}
impl From<SemanticsRegion> for Widget {
    fn from(w: SemanticsRegion) -> Self {
        Self::from_kind(WidgetKind::SemanticsRegion(w))
    }
}
impl From<Image> for Widget {
    fn from(w: Image) -> Self {
        Self::from_kind(WidgetKind::Image(w))
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
        Self::from_kind(WidgetKind::Video(w))
    }
}
impl From<ZStack> for Widget {
    fn from(w: ZStack) -> Self {
        Self::from_kind(WidgetKind::ZStack(w))
    }
}
impl From<Overlay> for Widget {
    fn from(w: Overlay) -> Self {
        Self::from_kind(WidgetKind::Overlay(w))
    }
}
impl From<ContextMenuRegion> for Widget {
    fn from(w: ContextMenuRegion) -> Self {
        Self::from_kind(WidgetKind::ContextMenuRegion(w))
    }
}

impl From<Container> for Widget {
    fn from(w: Container) -> Self {
        Self::from_kind(WidgetKind::Container(w))
    }
}
impl From<GestureDetector> for Widget {
    fn from(w: GestureDetector) -> Self {
        Self::from_kind(WidgetKind::GestureDetector(w))
    }
}
impl From<Grid> for Widget {
    fn from(w: Grid) -> Self {
        Self::from_kind(WidgetKind::Grid(w))
    }
}
impl From<GridItem> for Widget {
    fn from(w: GridItem) -> Self {
        Self::from_kind(WidgetKind::GridItem(w))
    }
}
impl From<Responsive> for Widget {
    fn from(w: Responsive) -> Self {
        Self::from_kind(WidgetKind::Responsive(w))
    }
}
impl From<Checkbox> for Widget {
    fn from(w: Checkbox) -> Self {
        Self::from_kind(WidgetKind::Checkbox(w))
    }
}
impl From<Switch> for Widget {
    fn from(w: Switch) -> Self {
        Self::from_kind(WidgetKind::Switch(w))
    }
}
impl From<Radio> for Widget {
    fn from(w: Radio) -> Self {
        Self::from_kind(WidgetKind::Radio(w))
    }
}
impl From<SafeArea> for Widget {
    fn from(w: SafeArea) -> Self {
        Self::from_kind(WidgetKind::SafeArea(w))
    }
}
impl From<Composite> for Widget {
    fn from(w: Composite) -> Self {
        Self::from_kind(WidgetKind::Composite(w))
    }
}
impl From<Positioned> for Widget {
    fn from(w: Positioned) -> Self {
        Self::from_kind(WidgetKind::Positioned(w))
    }
}
impl From<Spacer> for Widget {
    fn from(w: Spacer) -> Self {
        Self::from_kind(WidgetKind::Spacer(w))
    }
}
impl From<Slider> for Widget {
    fn from(w: Slider) -> Self {
        Self::from_kind(WidgetKind::Slider(w))
    }
}
impl From<LazyColumn> for Widget {
    fn from(w: LazyColumn) -> Self {
        Self::from_kind(WidgetKind::LazyColumn(w))
    }
}
impl From<Icon> for Widget {
    fn from(w: Icon) -> Self {
        Self::from_kind(WidgetKind::Icon(w))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InternalRenderNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerWidget>>,
    /// Optional render object that participates in hit-testing, event handling,
    /// and painting.  When `None`, the node behaves exactly as before (lowering
    /// only via `LowerWidget`).
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

    fn child_id(parent: WidgetId, slot: u32, discriminator: u32) -> WidgetId {
        WidgetId::derived(parent.as_u128(), &[Widget::CHILD_ROLE, slot, discriminator])
    }

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

    #[test]
    fn identity_resolution_preserves_structural_ids_and_explicit_scopes() {
        let root_id = WidgetId::explicit("identity.snapshot.root");
        let explicit_id = WidgetId::explicit("identity.snapshot.explicit");
        let widget: Widget = Column {
            children: vec![
                Text::new("automatic").into(),
                Container::new(Row {
                    children: vec![Text::new("scoped").into()],
                    ..Default::default()
                })
                .id(explicit_id),
            ],
            ..Default::default()
        }
        .into();

        let resolved = widget.resolve_identities(root_id);
        let WidgetKind::Column(root) = resolved.kind.as_ref() else {
            panic!("expected resolved root column");
        };
        assert_eq!(root.id, Some(root_id));
        assert_eq!(
            root.children[0].declared_id(),
            Some(child_id(root_id, 0, 8))
        );
        assert_eq!(root.children[1].declared_id(), Some(explicit_id));

        let WidgetKind::Container(explicit) = root.children[1].kind.as_ref() else {
            panic!("expected explicit container");
        };
        let row = explicit.child.as_ref().expect("explicit container child");
        let row_id = child_id(explicit_id, 0, 3);
        assert_eq!(row.declared_id(), Some(row_id));
        let WidgetKind::Row(row) = row.kind.as_ref() else {
            panic!("expected scoped row");
        };
        assert_eq!(row.children[0].declared_id(), Some(child_id(row_id, 0, 8)));
    }

    #[test]
    fn composition_heavy_tree_resolves_without_large_by_value_frames() {
        let mut branch: Widget = TextInput {
            prefix: Some(Text::new("prefix").into()),
            suffix: Some(
                Button {
                    child: Some(Text::new("send").into()),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into();

        // This models the nested surface/section/row anatomy produced by
        // composed cards, forms, timelines, and assistant panes. In an
        // unoptimized Wasm build, the former consuming traversal retained a
        // large WidgetKind value in every recursive frame.
        for index in 0..96 {
            branch = Container::new(Column {
                children: vec![
                    Row {
                        children: vec![Text::new(format!("Section {index}")).into(), branch],
                        ..Default::default()
                    }
                    .into(),
                    Container::new(Text::new("supporting content")).into(),
                ],
                ..Default::default()
            })
            .into();
        }

        let root_id = WidgetId::explicit("identity.composition-heavy");
        let resolved = branch.resolve_identities(root_id);
        assert_eq!(resolved.declared_id(), Some(root_id));
        let mut cursor = &resolved;
        for _ in 0..96 {
            assert!(cursor.declared_id().is_some());
            let WidgetKind::Container(container) = cursor.kind.as_ref() else {
                panic!("expected nested surface container");
            };
            let column = container.child.as_ref().expect("surface content");
            let WidgetKind::Column(column) = column.kind.as_ref() else {
                panic!("expected section column");
            };
            let WidgetKind::Row(row) = column.children[0].kind.as_ref() else {
                panic!("expected section header row");
            };
            cursor = &row.children[1];
        }
        let WidgetKind::TextInput(input) = cursor.kind.as_ref() else {
            panic!("expected retained text input leaf");
        };
        assert!(input.id.is_some());
        assert!(input
            .prefix
            .as_ref()
            .is_some_and(|prefix| prefix.declared_id().is_some()));
        assert!(input
            .suffix
            .as_ref()
            .is_some_and(|suffix| suffix.declared_id().is_some()));
    }
}
