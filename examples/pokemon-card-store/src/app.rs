use crate::cart::{cart_service, CartService};
use crate::data::{self, CatalogResponse, StoreError, CATALOG_JOB};
use fission::prelude::*;

pub use crate::components::store_card_page::StoreCardPage;
pub use crate::components::store_home_page::StoreHomePage;

#[derive(Debug, Clone)]
pub struct StoreState {
    pub catalog: AsyncSnapshot<CatalogResponse, StoreError>,
    pub session_id: String,
    pub cart_items: Vec<String>,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            catalog: AsyncSnapshot::waiting(),
            session_id: String::new(),
            cart_items: Vec::new(),
        }
    }
}

impl GlobalState for StoreState {}

impl StoreState {
    pub fn for_session(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        Self {
            catalog: AsyncSnapshot::waiting(),
            cart_items: cart_service().load(&session_id).items,
            session_id,
        }
    }
}

#[fission_reducer(CatalogLoaded)]
pub fn on_catalog_loaded(state: &mut StoreState, ctx: &mut ReducerContext<StoreState>) {
    if let Some(catalog) = ctx.input.job_ok(CATALOG_JOB) {
        state.catalog = AsyncSnapshot::with_data(AsyncConnectionState::Done, catalog);
    }
}

#[fission_reducer(CatalogFailed)]
pub fn on_catalog_failed(state: &mut StoreState, ctx: &mut ReducerContext<StoreState>) {
    let error = ctx
        .input
        .job_err(CATALOG_JOB)
        .unwrap_or_else(|| StoreError {
            message: ctx
                .input
                .job_error_message(CATALOG_JOB)
                .unwrap_or("Unable to load the catalogue")
                .to_string(),
        });
    state.catalog = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}

#[fission_reducer(AddToCart)]
pub fn on_add_to_cart(state: &mut StoreState, slug: String) {
    if data::card_by_slug(&slug).is_some() {
        if state.session_id.is_empty() {
            state.cart_items.push(slug);
        } else {
            state.cart_items = cart_service().add_item(&state.session_id, &slug).items;
        }
    }
}
