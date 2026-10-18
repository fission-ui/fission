use crate::catalog::example_by_slug;
use crate::components::Workbench;
use crate::state::ShowcaseState;
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Debug)] pub(crate) ExampleDetailPage<ShowcaseState> { slug: String }
    |component, _state, _cx| {} => Workbench(example_by_slug(&component.slug))
}
