use crate::components::card_detail::CardDetail;
use crate::components::not_found::NotFound;
use crate::components::shell::StoreShell;
use crate::data;
use fission::prelude::*;

fission_component! {
    pub StoreCardPage<crate::app::StoreState> {
        slug: String,
    } |page, _state, _cx| {} =>
    StoreShell(match data::card_by_slug(&page.slug) {
        Some(card) => CardDetail(card.clone()),
        None => NotFound(page.slug.clone()),
    })
}
