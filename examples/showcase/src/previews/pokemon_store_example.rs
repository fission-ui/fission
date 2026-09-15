use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use pokemon_card_store_example::{StoreHomePage, StoreState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PokemonStoreExample;

impl From<PokemonStoreExample> for Widget {
    fn from(_component: PokemonStoreExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        // The store is a web page taller than the preview; like a browser, the
        // preview scrolls it rather than squeezing every section to fit.
        Scroll {
            id: Some(WidgetId::explicit(
                "showcase.example.pokemon-card-store.scroll",
            )),
            child: Some(
                MountedExample::<StoreState, _>::new(
                    "showcase.example.pokemon-card-store",
                    view.state().preview_generation,
                    StoreHomePage,
                )
                .into(),
            ),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}
