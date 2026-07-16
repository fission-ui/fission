use std::collections::HashSet;

use fission_core::ScrollStateMap;
use fission_ir::WidgetId;

#[test]
fn scroll_state_discards_unmounted_scroll_nodes() {
    let active = WidgetId::explicit("active-scroll");
    let inactive = WidgetId::explicit("inactive-scroll");
    let mut scroll = ScrollStateMap::default();
    scroll.set_offset(active, 40.0);
    scroll.set_offset(inactive, 120.0);

    scroll.retain_active(&HashSet::from([active]));

    assert_eq!(scroll.get_offset(active), 40.0);
    assert_eq!(scroll.get_offset(inactive), 0.0);
}
