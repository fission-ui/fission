use fission::i18n::Locale;
use fission::prelude::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub price: String,
    pub active: bool,
    pub notes: String,
    pub saves: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub notifications: bool,
    pub compact_rows: bool,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub current_path: String,
    pub products: BTreeMap<u64, Product>,
    pub settings: Settings,
    pub locale: Locale,
    pub last_saved_product: Option<u64>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_path: "/".into(),
            products: [
                Product {
                    id: 101,
                    name: "Oak desk".into(),
                    price: "899.00".into(),
                    active: true,
                    notes: String::new(),
                    saves: 0,
                },
                Product {
                    id: 202,
                    name: "Task lamp".into(),
                    price: "125.00".into(),
                    active: true,
                    notes: String::new(),
                    saves: 0,
                },
                Product {
                    id: 303,
                    name: "Desk shelf".into(),
                    price: "199.00".into(),
                    active: false,
                    notes: String::new(),
                    saves: 0,
                },
            ]
            .into_iter()
            .map(|product| (product.id, product))
            .collect(),
            settings: Settings {
                notifications: true,
                compact_rows: false,
            },
            locale: Locale::from("en-US"),
            last_saved_product: None,
        }
    }
}

impl GlobalState for AppState {}

state_scope!(pub AppState => AppState);
state_scope! {
    pub Products(AppState, u64 => Product) {
        get: |root, key| root.products.get(key),
        get_mut: |root, key| root.products.get_mut(key),
    }
}
