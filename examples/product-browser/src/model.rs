use crate::api::{
    ApiError, CategoriesRequest, Product, ProductCategory, ProductPage, ProductRequest,
    CATEGORIES_JOB, PRODUCTS_JOB,
};
use fission::prelude::*;

const PRODUCT_LIMIT: u32 = 30;
const REFRESH_TRIGGER: f32 = 80.0;

#[derive(Debug, Clone)]
pub struct ProductBrowserState {
    pub query: String,
    pub selected_category: Option<String>,
    pub selected_product_id: Option<u64>,
    pub products: AsyncSnapshot<ProductPage, ApiError>,
    pub categories: AsyncSnapshot<Vec<ProductCategory>, ApiError>,
    pub product_generation: u64,
    pub category_generation: u64,
    pub refresh_status: RefreshIndicatorStatus,
    pub pulled_extent: f32,
}

impl Default for ProductBrowserState {
    fn default() -> Self {
        Self {
            query: String::new(),
            selected_category: None,
            selected_product_id: None,
            products: AsyncSnapshot::waiting(),
            categories: AsyncSnapshot::waiting(),
            product_generation: 0,
            category_generation: 0,
            refresh_status: RefreshIndicatorStatus::Inactive,
            pulled_extent: 0.0,
        }
    }
}

impl ProductBrowserState {
    pub fn product_request(&self) -> ProductRequest {
        ProductRequest {
            query: self.query.clone(),
            category: self.selected_category.clone(),
            limit: PRODUCT_LIMIT,
            refresh_generation: self.product_generation,
        }
    }

    pub fn categories_request(&self) -> CategoriesRequest {
        CategoriesRequest {
            refresh_generation: self.category_generation,
        }
    }

    pub fn selected_product(&self) -> Option<Product> {
        let page = self.products.data()?;
        let id = self.selected_product_id?;
        page.products
            .iter()
            .find(|product| product.id == id)
            .cloned()
    }

    fn restart_products(&mut self) {
        self.product_generation = self.product_generation.saturating_add(1);
        self.products = AsyncSnapshot::waiting();
    }
}

impl GlobalState for ProductBrowserState {}

#[fission_reducer(SearchChanged)]
pub fn on_search_changed(state: &mut ProductBrowserState, query: String) {
    state.query = query;
    state.selected_category = None;
    state.selected_product_id = None;
    state.refresh_status = RefreshIndicatorStatus::Inactive;
    state.pulled_extent = 0.0;
    state.restart_products();
}

#[fission_reducer(CategorySelected)]
pub fn on_category_selected(state: &mut ProductBrowserState, category: Option<String>) {
    state.selected_category = category;
    state.selected_product_id = None;
    state.refresh_status = RefreshIndicatorStatus::Inactive;
    state.pulled_extent = 0.0;
    state.restart_products();
}

#[fission_reducer(ProductSelected)]
pub fn on_product_selected(state: &mut ProductBrowserState, product_id: u64) {
    state.selected_product_id = Some(product_id);
}

#[fission_reducer(ProductsLoaded)]
pub fn on_products_loaded(
    state: &mut ProductBrowserState,
    ctx: &mut ReducerContext<ProductBrowserState>,
) {
    if let Some(page) = ctx.input.job_ok(PRODUCTS_JOB) {
        if state.selected_product_id.is_none() {
            state.selected_product_id = page.products.first().map(|product| product.id);
        }
        state.products = AsyncSnapshot::with_data(AsyncConnectionState::Done, page);
        state.refresh_status = RefreshIndicatorStatus::Inactive;
        state.pulled_extent = 0.0;
    }
}

#[fission_reducer(ProductsFailed)]
pub fn on_products_failed(
    state: &mut ProductBrowserState,
    ctx: &mut ReducerContext<ProductBrowserState>,
) {
    let error = ctx.input.job_err(PRODUCTS_JOB).unwrap_or_else(|| ApiError {
        message: ctx
            .input
            .job_error_message(PRODUCTS_JOB)
            .unwrap_or("Unable to load products")
            .to_string(),
    });
    state.products = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
    state.refresh_status = RefreshIndicatorStatus::Inactive;
    state.pulled_extent = 0.0;
}

#[fission_reducer(CategoriesLoaded)]
pub fn on_categories_loaded(
    state: &mut ProductBrowserState,
    ctx: &mut ReducerContext<ProductBrowserState>,
) {
    if let Some(categories) = ctx.input.job_ok(CATEGORIES_JOB) {
        state.categories = AsyncSnapshot::with_data(AsyncConnectionState::Done, categories);
    }
}

#[fission_reducer(CategoriesFailed)]
pub fn on_categories_failed(
    state: &mut ProductBrowserState,
    ctx: &mut ReducerContext<ProductBrowserState>,
) {
    let error = ctx
        .input
        .job_err(CATEGORIES_JOB)
        .unwrap_or_else(|| ApiError {
            message: ctx
                .input
                .job_error_message(CATEGORIES_JOB)
                .unwrap_or("Unable to load categories")
                .to_string(),
        });
    state.categories = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}

#[fission_reducer(PullStarted)]
pub fn on_pull_started(state: &mut ProductBrowserState) {
    if state.refresh_status != RefreshIndicatorStatus::Refreshing {
        state.refresh_status = RefreshIndicatorStatus::Drag;
        state.pulled_extent = 0.0;
    }
}

#[fission_reducer(PullUpdated)]
pub fn on_pull_updated(
    state: &mut ProductBrowserState,
    ctx: &mut ReducerContext<ProductBrowserState>,
) {
    if state.refresh_status == RefreshIndicatorStatus::Refreshing {
        return;
    }

    let Some((_, _, _, delta_y)) = ctx.input.as_pointer() else {
        return;
    };

    state.pulled_extent = (state.pulled_extent + delta_y).max(0.0);
    state.refresh_status = if state.pulled_extent >= REFRESH_TRIGGER {
        RefreshIndicatorStatus::Armed
    } else if state.pulled_extent > 0.0 {
        RefreshIndicatorStatus::Drag
    } else {
        RefreshIndicatorStatus::Inactive
    };
}

#[fission_reducer(PullCanceled)]
pub fn on_pull_canceled(state: &mut ProductBrowserState) {
    if state.refresh_status != RefreshIndicatorStatus::Refreshing {
        state.refresh_status = RefreshIndicatorStatus::Inactive;
        state.pulled_extent = 0.0;
    }
}

#[fission_reducer(RefreshProducts)]
pub fn on_refresh_products(state: &mut ProductBrowserState) {
    state.refresh_status = RefreshIndicatorStatus::Refreshing;
    state.pulled_extent = REFRESH_TRIGGER;
    state.restart_products();
}
