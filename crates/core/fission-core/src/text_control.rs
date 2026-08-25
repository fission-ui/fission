//! Programmatic control of retained editable text sessions.

use crate::{RuntimeState, TextEditingValue, TextRange, TextSelection};
use fission_ir::{op::FlexDirection, CoreIR, LayoutOp, Op, Role, WidgetId};
use fission_layout::LayoutSnapshot;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextEditingCommand {
    Focus,
    Unfocus,
    SelectAll,
    SetSelection(TextSelection),
    SetValue(TextEditingValue),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextEditingController {
    id: WidgetId,
}

impl TextEditingController {
    pub const fn new(id: WidgetId) -> Self {
        Self { id }
    }

    pub const fn id(self) -> WidgetId {
        self.id
    }

    pub fn value(self, state: &RuntimeState) -> Option<TextEditingValue> {
        state
            .text_edit
            .get(self.id)
            .map(|state| state.editing_value())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextScrollCommand {
    Caret,
    Range(TextRange),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextScrollController {
    id: WidgetId,
}

impl TextScrollController {
    pub const fn new(id: WidgetId) -> Self {
        Self { id }
    }

    pub const fn id(self) -> WidgetId {
        self.id
    }

    pub fn apply(
        self,
        state: &mut RuntimeState,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
        command: TextScrollCommand,
    ) -> Result<(), TextControlError> {
        let (scroll_id, text_id, direction) =
            text_scroll_nodes(ir, self.id).ok_or(TextControlError::MissingTextInput(self.id))?;
        let index = match command {
            TextScrollCommand::Caret => state
                .text_edit
                .get(self.id)
                .map(|value| value.caret)
                .unwrap_or(0),
            TextScrollCommand::Range(range) => range.end.utf8_offset(),
        };
        let paragraph = layout
            .get_resolved_paragraph(text_id)
            .ok_or(TextControlError::ParagraphNotResolved(self.id))?;
        let caret = paragraph
            .caret(index, false)
            .ok_or(TextControlError::InvalidRange(self.id))?;
        let viewport = layout
            .get_node_geometry(scroll_id)
            .ok_or(TextControlError::ParagraphNotResolved(self.id))?;
        let current = state.scroll.get_offset(scroll_id);
        let (leading, trailing, viewport_extent, content_extent) = match direction {
            FlexDirection::Row => (
                caret.position.x,
                caret.position.x + 2.0,
                viewport.rect.width(),
                viewport.content_size.width,
            ),
            FlexDirection::Column => (
                caret.position.y,
                caret.position.y + caret.height.max(1.0),
                viewport.rect.height(),
                viewport.content_size.height,
            ),
        };
        let mut offset = current;
        if leading < current {
            offset = leading;
        } else if trailing > current + viewport_extent {
            offset = trailing - viewport_extent;
        }
        state.scroll.set_offset(
            scroll_id,
            offset.clamp(0.0, (content_extent - viewport_extent).max(0.0)),
        );
        Ok(())
    }
}

fn text_scroll_nodes(ir: &CoreIR, root: WidgetId) -> Option<(WidgetId, WidgetId, FlexDirection)> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = ir.nodes.get(&id)?;
        if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
            let mut descendants = node.children.clone();
            while let Some(child) = descendants.pop() {
                let child_node = ir.nodes.get(&child)?;
                if matches!(
                    child_node.op,
                    Op::Paint(fission_ir::PaintOp::DrawText { .. })
                        | Op::Paint(fission_ir::PaintOp::DrawRichText { .. })
                ) {
                    return Some((id, child, *direction));
                }
                descendants.extend(child_node.children.iter().copied());
            }
        }
        stack.extend(node.children.iter().copied());
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextControlError {
    MissingTextInput(WidgetId),
    InvalidRange(WidgetId),
    ParagraphNotResolved(WidgetId),
}

impl fmt::Display for TextControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTextInput(id) => write!(formatter, "text input {id} is not in the tree"),
            Self::InvalidRange(id) => {
                write!(formatter, "text input {id} received an invalid range")
            }
            Self::ParagraphNotResolved(id) => {
                write!(formatter, "text input {id} has no resolved paragraph")
            }
        }
    }
}

impl Error for TextControlError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextFormController {
    id: String,
}

impl TextFormController {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn validation(&self, ir: &CoreIR) -> TextFormValidation {
        let mut fields = Vec::new();
        let mut invalid = Vec::new();
        for (node_id, node) in &ir.nodes {
            let Op::Semantics(semantics) = &node.op else {
                continue;
            };
            if semantics.role != Role::TextInput
                || semantics.text_form_id.as_deref() != Some(self.id.as_str())
            {
                continue;
            }
            fields.push(*node_id);
            if semantics.validation_state
                == fission_ir::semantics::TextFieldValidationState::Invalid
            {
                invalid.push(*node_id);
            }
        }
        TextFormValidation { fields, invalid }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFormValidation {
    pub fields: Vec<WidgetId>,
    pub invalid: Vec<WidgetId>,
}

impl TextFormValidation {
    pub fn is_valid(&self) -> bool {
        self.invalid.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_ir::{CompositeStyle, CoreNode, Semantics};

    fn text_input(ir: &mut CoreIR, id: WidgetId, value: &str) {
        ir.nodes.insert(
            id,
            CoreNode {
                id,
                parent: None,
                children: Vec::new(),
                op: Op::Semantics(Semantics {
                    role: Role::TextInput,
                    value: Some(value.to_owned()),
                    focusable: true,
                    ..Semantics::default()
                }),
                composite: CompositeStyle::default(),
                hash: 0,
            },
        );
    }

    #[test]
    fn editing_controller_targets_one_stable_widget() {
        let first = WidgetId::explicit("form.first");
        let second = WidgetId::explicit("form.second");
        let mut ir = CoreIR::default();
        text_input(&mut ir, first, "alpha");
        text_input(&mut ir, second, "beta");
        let mut state = RuntimeState::default();
        let controller = TextEditingController::new(first);
        state
            .text_edit
            .sync_from_runtime(first, "alpha", None, None, false);
        state.text_edit.set_caret(first, 5, Some(0));

        assert_eq!(
            controller.value(&state).unwrap().selection_range(),
            TextRange::new("alpha", 0, 5).unwrap()
        );
        assert!(state.text_edit.get(second).is_none());
    }

    #[test]
    fn form_controller_reports_only_invalid_members_of_its_form() {
        let valid = WidgetId::explicit("account.name");
        let invalid = WidgetId::explicit("account.email");
        let unrelated = WidgetId::explicit("search.query");
        let mut ir = CoreIR::default();
        for (id, form, state) in [
            (
                valid,
                "account",
                fission_ir::semantics::TextFieldValidationState::Valid,
            ),
            (
                invalid,
                "account",
                fission_ir::semantics::TextFieldValidationState::Invalid,
            ),
            (
                unrelated,
                "search",
                fission_ir::semantics::TextFieldValidationState::Invalid,
            ),
        ] {
            text_input(&mut ir, id, "value");
            let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&id).unwrap().op else {
                unreachable!();
            };
            semantics.text_form_id = Some(form.to_owned());
            semantics.validation_state = state;
        }

        let result = TextFormController::new("account").validation(&ir);
        assert_eq!(result.fields.len(), 2);
        assert_eq!(result.invalid, vec![invalid]);
        assert!(!result.is_valid());
    }
}
