use crate::catalog::example_by_slug;
use crate::components::Workbench;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct ExampleDetailPage {
    pub(crate) slug: String,
}

impl From<ExampleDetailPage> for Widget {
    fn from(component: ExampleDetailPage) -> Self {
        Workbench {
            example: example_by_slug(&component.slug),
        }
        .into()
    }
}
