//! Coordinated selection across read-only text descendants.

use crate::env::RuntimeState;
use crate::{TextAffinity, TextPosition};
use fission_ir::{CoreIR, Op, WidgetId};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// A position in one selectable descendant of a [`SelectionRegion`](crate::ui::SelectionRegion).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRegionPosition {
    pub node_id: WidgetId,
    pub offset: TextPosition,
}

impl TextRegionPosition {
    /// Creates a region position after validating its UTF-8 offset.
    pub fn new(
        node_id: WidgetId,
        text: &str,
        offset: usize,
    ) -> Result<Self, crate::text_editing::TextOffsetError> {
        Ok(Self {
            node_id,
            offset: TextPosition::from_utf8(text, offset)?,
        })
    }

    pub const fn at(node_id: WidgetId, offset: TextPosition) -> Self {
        Self { node_id, offset }
    }
}

/// A directional selection which can span several read-only text nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRegionSelection {
    pub base: TextRegionPosition,
    pub extent: TextRegionPosition,
    pub affinity: TextAffinity,
}

impl TextRegionSelection {
    pub const fn collapsed(at: TextRegionPosition) -> Self {
        Self {
            base: at,
            extent: at,
            affinity: TextAffinity::Downstream,
        }
    }

    pub fn is_collapsed(self) -> bool {
        self.base.node_id == self.extent.node_id
            && self.base.offset.utf8_offset() == self.extent.offset.utf8_offset()
    }
}

/// Programmatic operation for a selection region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionRegionCommand {
    Clear,
    SelectAll,
    Select(TextRegionSelection),
}

/// Stable handle used to inspect or update a selection region's retained state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SelectionRegionController {
    id: WidgetId,
}

impl SelectionRegionController {
    pub const fn new(id: WidgetId) -> Self {
        Self { id }
    }

    pub const fn id(self) -> WidgetId {
        self.id
    }

    /// Reads the retained selection, for example from `ViewHandle::runtime()`
    /// during a declarative build.
    pub fn selection(self, state: &RuntimeState) -> Option<TextRegionSelection> {
        state.selectable_text.region_selection(self.id)
    }

    /// Applies a selection command to the current lowered tree.
    pub fn apply(
        self,
        state: &mut RuntimeState,
        ir: &CoreIR,
        command: SelectionRegionCommand,
    ) -> Result<(), SelectionRegionError> {
        apply_region_command(&mut state.selectable_text, ir, self.id, command)
    }

    /// Selects a range expressed in Unicode scalar offsets into the region's
    /// combined accessibility value.
    pub fn select_scalar_range(
        self,
        state: &mut RuntimeState,
        ir: &CoreIR,
        base: usize,
        extent: usize,
        affinity: TextAffinity,
    ) -> Result<(), SelectionRegionError> {
        let document =
            region_document(ir, self.id).ok_or(SelectionRegionError::MissingRegion(self.id))?;
        let base = TextPosition::from_scalar_offset(&document.text, base).map_err(|_| {
            SelectionRegionError::InvalidOffset {
                node: self.id,
                offset: base,
            }
        })?;
        let extent = TextPosition::from_scalar_offset(&document.text, extent).map_err(|_| {
            SelectionRegionError::InvalidOffset {
                node: self.id,
                offset: extent,
            }
        })?;
        let base = document
            .position_at(base.utf8_offset())
            .ok_or(SelectionRegionError::EmptyRegion(self.id))?;
        let extent = document
            .position_at(extent.utf8_offset())
            .ok_or(SelectionRegionError::EmptyRegion(self.id))?;
        set_region_selection(
            &mut state.selectable_text,
            self.id,
            &document,
            TextRegionSelection {
                base,
                extent,
                affinity,
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionRegionError {
    MissingRegion(WidgetId),
    NotAMember { region: WidgetId, node: WidgetId },
    InvalidOffset { node: WidgetId, offset: usize },
    EmptyRegion(WidgetId),
}

impl fmt::Display for SelectionRegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRegion(id) => {
                write!(f, "selection region {id} is not in the current tree")
            }
            Self::NotAMember { region, node } => {
                write!(
                    f,
                    "text node {node} is not a member of selection region {region}"
                )
            }
            Self::InvalidOffset { node, offset } => {
                write!(
                    f,
                    "offset {offset} is invalid for selectable text node {node}"
                )
            }
            Self::EmptyRegion(id) => write!(f, "selection region {id} has no selectable text"),
        }
    }
}

impl Error for SelectionRegionError {}

#[derive(Clone, Debug)]
pub(crate) struct RegionDocument {
    pub text: String,
    pub members: Vec<RegionMember>,
}

#[derive(Clone, Debug)]
pub(crate) struct RegionMember {
    pub node_id: WidgetId,
    pub text: String,
    pub document_start: usize,
}

impl RegionDocument {
    pub fn position_offset(&self, position: TextRegionPosition) -> Option<usize> {
        let member = self
            .members
            .iter()
            .find(|member| member.node_id == position.node_id)?;
        let local = position.offset.utf8_offset();
        (local <= member.text.len() && member.text.is_char_boundary(local))
            .then_some(member.document_start + local)
    }

    fn position_at(&self, document_offset: usize) -> Option<TextRegionPosition> {
        let offset = document_offset.min(self.text.len());
        let member = self
            .members
            .iter()
            .rev()
            .find(|member| member.document_start <= offset)?;
        let local = offset
            .saturating_sub(member.document_start)
            .min(member.text.len());
        Some(TextRegionPosition::at(
            member.node_id,
            TextPosition::floor(&member.text, local),
        ))
    }

    pub fn selected_text(&self, selection: TextRegionSelection) -> Option<String> {
        let base = self.position_offset(selection.base)?;
        let extent = self.position_offset(selection.extent)?;
        let start = base.min(extent);
        let end = base.max(extent);
        self.text.get(start..end).map(ToOwned::to_owned)
    }
}

pub(crate) fn region_document(ir: &CoreIR, region_id: WidgetId) -> Option<RegionDocument> {
    let node = ir.nodes.get(&region_id)?;
    let semantics = match &node.op {
        Op::Semantics(semantics) => semantics,
        _ => return None,
    };
    let config = semantics.selection_region.as_ref()?;
    if config.excluded {
        return None;
    }
    Some(document_from_members(
        selectable_members(ir, region_id),
        &config.separator,
        ir,
    ))
}

pub(crate) fn implicit_document(ir: &CoreIR, node_id: WidgetId) -> Option<RegionDocument> {
    let semantics = selectable_semantics(ir, node_id)?;
    Some(document_from_values(
        [(node_id, semantics.value.clone().unwrap_or_default())],
        "",
    ))
}

pub(crate) fn document_for_selection_owner(ir: &CoreIR, owner: WidgetId) -> Option<RegionDocument> {
    region_document(ir, owner).or_else(|| implicit_document(ir, owner))
}

pub(crate) fn selectable_members(ir: &CoreIR, region_id: WidgetId) -> Vec<WidgetId> {
    let Some(region) = ir.nodes.get(&region_id) else {
        return Vec::new();
    };
    let mut members = Vec::new();
    for child in &region.children {
        collect_members(ir, *child, &mut members);
    }
    members
}

pub(crate) fn selectable_members_in_subtree(ir: &CoreIR, root: WidgetId) -> Vec<WidgetId> {
    let mut members = Vec::new();
    collect_members(ir, root, &mut members);
    members
}

fn collect_members(ir: &CoreIR, node_id: WidgetId, members: &mut Vec<WidgetId>) {
    let Some(node) = ir.nodes.get(&node_id) else {
        return;
    };
    if let Op::Semantics(semantics) = &node.op {
        if semantics.selection_region.is_some() {
            return;
        }
        if semantics.selectable_text && !semantics.disabled {
            members.push(node_id);
            return;
        }
    }
    for child in &node.children {
        collect_members(ir, *child, members);
    }
}

fn document_from_members(members: Vec<WidgetId>, separator: &str, ir: &CoreIR) -> RegionDocument {
    document_from_values(
        members.into_iter().filter_map(|id| {
            selectable_semantics(ir, id)
                .map(|semantics| (id, semantics.value.clone().unwrap_or_default()))
        }),
        separator,
    )
}

fn document_from_values(
    values: impl IntoIterator<Item = (WidgetId, String)>,
    separator: &str,
) -> RegionDocument {
    let mut text = String::new();
    let mut members = Vec::new();
    for (index, (node_id, value)) in values.into_iter().enumerate() {
        if index > 0 {
            text.push_str(separator);
        }
        let document_start = text.len();
        text.push_str(&value);
        members.push(RegionMember {
            node_id,
            text: value,
            document_start,
        });
    }
    RegionDocument { text, members }
}

pub(crate) fn selectable_semantics(
    ir: &CoreIR,
    node_id: WidgetId,
) -> Option<&fission_ir::Semantics> {
    match &ir.nodes.get(&node_id)?.op {
        Op::Semantics(semantics) if semantics.selectable_text && !semantics.disabled => {
            Some(semantics)
        }
        _ => None,
    }
}

pub(crate) fn apply_region_command(
    states: &mut crate::env::SelectableTextStateMap,
    ir: &CoreIR,
    region_id: WidgetId,
    command: SelectionRegionCommand,
) -> Result<(), SelectionRegionError> {
    let document = document_for_selection_owner(ir, region_id)
        .ok_or(SelectionRegionError::MissingRegion(region_id))?;
    if document.members.is_empty() {
        return Err(SelectionRegionError::EmptyRegion(region_id));
    }
    match command {
        SelectionRegionCommand::Clear => {
            states.clear_region(region_id, &document);
            Ok(())
        }
        SelectionRegionCommand::SelectAll => {
            let first = &document.members[0];
            let last = document.members.last().expect("non-empty region");
            let selection = TextRegionSelection {
                base: TextRegionPosition::at(first.node_id, TextPosition::START),
                extent: TextRegionPosition::at(last.node_id, TextPosition::at_end(&last.text)),
                affinity: TextAffinity::Downstream,
            };
            set_region_selection(states, region_id, &document, selection)
        }
        SelectionRegionCommand::Select(selection) => {
            set_region_selection(states, region_id, &document, selection)
        }
    }
}

pub(crate) fn set_region_selection(
    states: &mut crate::env::SelectableTextStateMap,
    region_id: WidgetId,
    document: &RegionDocument,
    selection: TextRegionSelection,
) -> Result<(), SelectionRegionError> {
    for position in [selection.base, selection.extent] {
        let Some(member) = document
            .members
            .iter()
            .find(|member| member.node_id == position.node_id)
        else {
            return Err(SelectionRegionError::NotAMember {
                region: region_id,
                node: position.node_id,
            });
        };
        let offset = position.offset.utf8_offset();
        if offset > member.text.len() || !member.text.is_char_boundary(offset) {
            return Err(SelectionRegionError::InvalidOffset {
                node: position.node_id,
                offset,
            });
        }
    }

    let base = document
        .position_offset(selection.base)
        .expect("validated base");
    let extent = document
        .position_offset(selection.extent)
        .expect("validated extent");
    let start = base.min(extent);
    let end = base.max(extent);

    for member in &document.members {
        let member_start = member.document_start;
        let member_end = member_start + member.text.len();
        let local_start = start.max(member_start).min(member_end) - member_start;
        let local_end = end.max(member_start).min(member_end) - member_start;
        let state = states.states.entry(member.node_id).or_default();
        state.anchor = local_start;
        state.caret = local_end;
        state.selecting = false;
    }
    states.region_mut_or_default(region_id).selection = Some(selection);
    Ok(())
}

pub(crate) fn clear_other_regions(
    states: &mut crate::env::SelectableTextStateMap,
    ir: &CoreIR,
    active_region: WidgetId,
) {
    let stale_regions: Vec<WidgetId> = states
        .regions
        .keys()
        .copied()
        .filter(|id| *id != active_region)
        .collect();
    for region_id in stale_regions {
        if let Some(document) = document_for_selection_owner(ir, region_id) {
            states.clear_region(region_id, &document);
        } else if let Some(state) = states.regions.get_mut(&region_id) {
            state.selection = None;
            state.selecting = false;
        }
    }
}

pub(crate) fn reconcile_selection_state(
    states: &mut crate::env::SelectableTextStateMap,
    ir: &CoreIR,
) {
    let active_text: std::collections::HashSet<WidgetId> = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| match &node.op {
            Op::Semantics(semantics) if semantics.selectable_text && !semantics.disabled => {
                Some(*id)
            }
            _ => None,
        })
        .collect();
    states.states.retain(|id, _| active_text.contains(id));

    let active_regions: std::collections::HashSet<WidgetId> = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| match &node.op {
            Op::Semantics(semantics)
                if semantics
                    .selection_region
                    .as_ref()
                    .is_some_and(|region| !region.excluded) =>
            {
                Some(*id)
            }
            Op::Semantics(semantics) if semantics.selectable_text && !semantics.disabled => {
                Some(*id)
            }
            _ => None,
        })
        .collect();
    states.regions.retain(|id, _| active_regions.contains(id));

    let retained: Vec<(WidgetId, TextRegionSelection)> = states
        .regions
        .iter()
        .filter_map(|(id, state)| state.selection.map(|selection| (*id, selection)))
        .collect();
    for (region_id, selection) in retained {
        let Some(document) = document_for_selection_owner(ir, region_id) else {
            states.regions.remove(&region_id);
            continue;
        };
        if set_region_selection(states, region_id, &document, selection).is_err() {
            states.clear_region(region_id, &document);
        }
    }
}
