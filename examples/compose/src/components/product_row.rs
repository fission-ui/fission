use crate::actions::{navigate, Navigate};
use crate::components::StatusPill;
use crate::state::{Product, Products};
use fission::prelude::*;

Component! {
    pub ProductRow<Product> using Products(row.product_id) {
        product_id: u64,
    } |row, product, cx| =>
    Row(children: [
        Text(product.name.clone()),
        Text(cx.tr("products.price").replace("{price}", &product.price.to_string())),
        StatusPill(
            text: if product.active { cx.tr("products.available") } else { cx.tr("products.draft") },
            active: product.active,
        ),
        Button(
            child: Text(cx.tr("products.open")),
            key: "open",
            variant: ButtonVariant::Outline,
            on_press: cx.bind(
                Navigate(format!("/products/{}", row.product_id)),
                reduce!(navigate),
            ),
        ),
    ])
}
