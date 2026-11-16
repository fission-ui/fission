//! The preview lowers with the design system picked for it, so its controls line
//! up the way that design system lays them out rather than mixing in the
//! showcase's own recipe values.

use crate::app::ShowcaseApp;
use crate::i18n::create_env;
use crate::state::{DesignSystemChoice, ShowcaseState};
use fission::layout::LayoutSize;
use fission::prelude::WidgetId;
use fission_core::op::Op;
use fission_core::Role;
use fission_test::TestHarness;

#[test]
fn a_material_3_preview_lines_up_its_select_with_its_buttons() {
    let state = ShowcaseState {
        current_path: "/examples/inbox".into(),
        design_system: DesignSystemChoice::Material3,
        ..ShowcaseState::default()
    };
    let mut h = TestHarness::new(state).with_root_widget(ShowcaseApp);
    h.env = create_env().expect("showcase environment");
    h.env.viewport_size = LayoutSize::new(1280.0, 860.0);
    // The preview reads its height from the previous frame's layout, so settle it.
    h.pump().expect("first frame");
    h.pump().expect("settled frame");
    let ir = h.last_ir.as_ref().expect("lowered IR");
    let snapshot = h.last_snapshot.as_ref().expect("layout");

    let shows_text = |root: WidgetId, needle: &str| {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = ir.nodes.get(&id) else {
                continue;
            };
            if node.op.text().is_some_and(|text| text == needle) {
                return true;
            }
            stack.extend(node.children.iter().copied());
        }
        false
    };
    let height_of = |role: Role, label: &str| {
        let (id, node) = ir
            .nodes
            .iter()
            .find(|(id, node)| {
                matches!(&node.op, Op::Semantics(semantics) if semantics.role == role)
                    && shows_text(**id, label)
            })
            .unwrap_or_else(|| panic!("a {role:?} showing {label:?} in the preview"));
        let layout = node.children.first().copied().unwrap_or(*id);
        snapshot
            .get_node_rect(layout)
            .unwrap_or_else(|| panic!("{label:?} is laid out"))
            .height()
    };

    let select = height_of(Role::ComboBox, "Newest");
    let filters = height_of(Role::Button, "Filters");
    assert!(
        (select - filters).abs() < 0.5,
        "in Material 3 the sort select is {select}px beside a {filters}px Filters button"
    );
}
