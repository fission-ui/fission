use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use product_browser_example::{ProductBrowserApp, ProductBrowserState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductBrowserExample;

impl From<ProductBrowserExample> for Widget {
    fn from(_component: ProductBrowserExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<ProductBrowserState, _>::new(
            "showcase.example.product-browser",
            view.state().preview_generation,
            ProductBrowserApp,
        )
        .into()
    }
}
