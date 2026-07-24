use crate::components::card_detail::CardDetail;
use crate::components::not_found::NotFound;
use crate::components::shell::StoreShell;
use crate::data;
use fission::prelude::*;

#[derive(Clone)]
pub struct StoreCardPage {
    pub slug: String,
}

impl From<StoreCardPage> for Widget {
    fn from(page: StoreCardPage) -> Self {
        let child: Widget = match data::card_by_slug(&page.slug) {
            Some(card) => CardDetail { card: card.clone() }.into(),
            None => NotFound { slug: page.slug }.into(),
        };

        StoreShell { child }.into()
    }
}
