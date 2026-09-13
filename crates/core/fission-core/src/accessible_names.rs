//! Names controls from their visible text when the author gave them no label, as a browser names a
//! button from its contents.

use fission_ir::{CoreIR, Op, Role, WidgetId};

/// Gives an unlabelled control a label made of the text beneath it, stopping at nested controls,
/// which name themselves. Called before the node is inserted, so its hash covers the label.
pub(crate) fn name_from_content(ir: &CoreIR, op: &mut Op, children: &[WidgetId]) {
    let Op::Semantics(semantics) = op else {
        return;
    };
    if semantics.label.is_some() || !named_from_content(semantics.role) {
        return;
    }
    let mut parts = Vec::new();
    for child in children {
        collect_text(ir, *child, &mut parts);
    }
    let name = parts
        .iter()
        .flat_map(|part| part.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
    if !name.is_empty() {
        semantics.label = Some(name);
    }
}

fn named_from_content(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::MenuItem
            | Role::Tab
            | Role::Option
            | Role::TreeItem
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
    )
}

fn collect_text(ir: &CoreIR, id: WidgetId, parts: &mut Vec<String>) {
    let Some(node) = ir.nodes.get(&id) else {
        return;
    };
    if let Op::Semantics(semantics) = &node.op {
        if named_from_content(semantics.role)
            || matches!(
                semantics.role,
                Role::TextInput | Role::Input | Role::Slider | Role::ComboBox
            )
        {
            return;
        }
    }
    if let Some(text) = node.op.text() {
        parts.push(text.into_owned());
    }
    for child in &node.children {
        collect_text(ir, *child, parts);
    }
}
