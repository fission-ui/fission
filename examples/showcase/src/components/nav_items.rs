use super::nav_item::NavItem;
use crate::state::ShowcaseState;
use fission::op::AlignItems;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct NavItems;

impl From<NavItems> for Widget {
    fn from(_component: NavItems) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        Row {
            children: widgets![NavItem {
                label_key: "showcase.nav.discover",
                path: "/",
            },],
            gap: Some(tokens.spacing.xs),
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into()
    }
}
