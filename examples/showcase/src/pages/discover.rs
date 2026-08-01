use crate::catalog::example_by_slug;
use crate::components::Workbench;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct DiscoverPage;

impl From<DiscoverPage> for Widget {
    fn from(_component: DiscoverPage) -> Self {
        Workbench {
            example: example_by_slug("inbox"),
        }
        .into()
    }
}
