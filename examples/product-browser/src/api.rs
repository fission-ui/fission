use fission::core::{JobRef, JobSpec};
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://dummyjson.com";
const USER_AGENT: &str = "FissionProductBrowser/0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    pub message: String,
}

impl ApiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::new(error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductRequest {
    pub query: String,
    pub category: Option<String>,
    pub limit: u32,
    pub refresh_generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductPage {
    pub products: Vec<Product>,
    pub total: u32,
    pub skip: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub category: String,
    pub price: f64,
    pub discount_percentage: Option<f64>,
    pub rating: f64,
    pub stock: u32,
    pub brand: Option<String>,
    pub thumbnail: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductCategory {
    pub slug: String,
    pub name: String,
}

#[derive(Debug)]
pub struct ProductsJob;

impl JobSpec for ProductsJob {
    type Request = ProductRequest;
    type Ok = ProductPage;
    type Err = ApiError;

    const NAME: &'static str = "product-browser.products";
}

pub const PRODUCTS_JOB: JobRef<ProductsJob> = JobRef::new(ProductsJob::NAME);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoriesRequest {
    pub refresh_generation: u64,
}

#[derive(Debug)]
pub struct CategoriesJob;

impl JobSpec for CategoriesJob {
    type Request = CategoriesRequest;
    type Ok = Vec<ProductCategory>;
    type Err = ApiError;

    const NAME: &'static str = "product-browser.categories";
}

pub const CATEGORIES_JOB: JobRef<CategoriesJob> = JobRef::new(CategoriesJob::NAME);

pub async fn fetch_products(request: ProductRequest) -> Result<ProductPage, ApiError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(ApiError::from)?;

    let trimmed_query = request.query.trim();
    let limit = request.limit.clamp(1, 100).to_string();
    let response = if !trimmed_query.is_empty() {
        client
            .get(format!("{API_BASE}/products/search"))
            .query(&[("q", trimmed_query), ("limit", limit.as_str())])
            .send()
            .await?
    } else if let Some(category) = request
        .category
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        client
            .get(format!("{API_BASE}/products/category/{category}"))
            .query(&[("limit", limit.as_str())])
            .send()
            .await?
    } else {
        client
            .get(format!("{API_BASE}/products"))
            .query(&[("limit", limit.as_str())])
            .send()
            .await?
    };

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::new(format!(
            "product request failed with {status}"
        )));
    }

    response.json::<ProductPage>().await.map_err(ApiError::from)
}

pub async fn fetch_categories(
    _request: CategoriesRequest,
) -> Result<Vec<ProductCategory>, ApiError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(ApiError::from)?;
    let response = client
        .get(format!("{API_BASE}/products/categories"))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::new(format!(
            "category request failed with {status}"
        )));
    }

    response
        .json::<Vec<ProductCategory>>()
        .await
        .map_err(ApiError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product_with_thumbnail(thumbnail: &str) -> Product {
        Product {
            id: 42,
            title: "Demo".into(),
            description: "Demo product".into(),
            category: "demo".into(),
            price: 1.0,
            discount_percentage: None,
            rating: 4.0,
            stock: 10,
            brand: None,
            thumbnail: thumbnail.into(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn product_model_preserves_remote_thumbnail_url() {
        let product = product_with_thumbnail("https://cdn.example.com/products/42/thumbnail.webp");
        assert_eq!(
            product.thumbnail,
            "https://cdn.example.com/products/42/thumbnail.webp"
        );
    }
}
