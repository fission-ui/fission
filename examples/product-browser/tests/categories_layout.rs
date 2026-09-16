//! Layout regression: the narrow category list must show every category
//! instead of trapping them in a short scrolling card.

use fission::prelude::{AsyncConnectionState, AsyncSnapshot};
use fission_test::{op_text, TestHarness};
use product_browser::{translation_bundles, ProductBrowserApp, ProductBrowserState};

const WIDTH: f32 = 880.0;
const HEIGHT: f32 = 660.0;

const CATEGORIES: [&str; 12] = [
    "Beauty",
    "Fragrances",
    "Furniture",
    "Groceries",
    "Home Decoration",
    "Kitchen Accessories",
    "Laptops",
    "Mens Shirts",
    "Mens Shoes",
    "Mens Watches",
    "Mobile Accessories",
    "Motorcycle",
];

fn state() -> ProductBrowserState {
    let categories = CATEGORIES
        .iter()
        .map(|name| product_browser::ProductCategory {
            slug: name.to_lowercase().replace(' ', "-"),
            name: (*name).to_string(),
        })
        .collect();
    ProductBrowserState {
        categories: AsyncSnapshot::with_data(AsyncConnectionState::Done, categories),
        ..ProductBrowserState::default()
    }
}

fn pump(state: ProductBrowserState) -> TestHarness<ProductBrowserState> {
    let mut harness = TestHarness::new(state).with_root_widget(ProductBrowserApp);
    for bundle in translation_bundles() {
        harness.env.i18n.add_bundle(bundle);
    }
    harness.env.locale = fission::i18n::Locale::from("en-US");
    harness.env.viewport_size = fission::layout::LayoutSize::new(WIDTH, HEIGHT);
    harness.pump().expect("pump product browser");
    harness
}

fn title_count(harness: &TestHarness<ProductBrowserState>) -> usize {
    let ir = harness.last_ir.as_ref().expect("ir");
    ir.nodes
        .iter()
        .filter(|(_, node)| op_text(&node.op).is_some_and(|text| text == "Product Browser"))
        .count()
}

/// Embedded in the showcase, whose header already names the example, the
/// browser keeps its result summary but drops the app title.
#[test]
fn embedded_browser_drops_its_app_title() {
    assert_eq!(title_count(&pump(ProductBrowserState::default())), 1);
    assert_eq!(title_count(&pump(product_browser::embedded_state())), 0);
}

#[test]
fn compact_categories_are_all_visible_without_scrolling() {
    let harness = pump(state());

    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    for name in std::iter::once("All products").chain(CATEGORIES) {
        let rect = ir
            .nodes
            .iter()
            .filter(|(_, node)| op_text(&node.op).is_some_and(|text| text == name))
            .find_map(|(id, _)| snapshot.get_node_rect(*id))
            .unwrap_or_else(|| panic!("category {name:?} is not laid out"));
        assert!(
            rect.size.height > 0.0 && rect.right() <= WIDTH + 0.5 && rect.bottom() <= HEIGHT * 0.6,
            "category {name:?} ({rect:?}) is clipped or pushed out of the header area"
        );
    }
}
