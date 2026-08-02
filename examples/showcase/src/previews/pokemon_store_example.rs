use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use pokemon_card_store_example::{StoreHomePage, StoreState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PokemonStoreExample;

impl From<PokemonStoreExample> for Widget {
    fn from(_component: PokemonStoreExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<StoreState, _>::new(
            "showcase.example.pokemon-card-store",
            view.state().preview_generation,
            StoreHomePage,
        )
        .into()
    }
}
