use fission::core::{JobRef, JobSpec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub slug: &'static str,
    pub name: &'static str,
    pub set: &'static str,
    pub type_line: &'static str,
    pub rarity: &'static str,
    pub price: f32,
    pub stock: u32,
    pub accent: (u8, u8, u8),
    pub description: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRequest {
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogResponse {
    pub cards: Vec<CardSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CardSummary {
    pub slug: String,
    pub name: String,
    pub price: f32,
    pub stock: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreError {
    pub message: String,
}

pub struct CatalogJob;

impl JobSpec for CatalogJob {
    type Request = CatalogRequest;
    type Ok = CatalogResponse;
    type Err = StoreError;

    const NAME: &'static str = "pokemon-card-store.catalog";
}

pub const CATALOG_JOB: JobRef<CatalogJob> = JobRef::new(CatalogJob::NAME);

pub fn cards() -> &'static [Card] {
    &[
        Card {
            slug: "charizard-holo",
            name: "Charizard Holo",
            set: "Base Set",
            type_line: "Fire / Stage 2",
            rarity: "Holo Rare",
            price: 249.00,
            stock: 3,
            accent: (245, 94, 61),
            description: "The headline card for collectors who want a dramatic centrepiece.",
        },
        Card {
            slug: "pikachu-yellow-cheeks",
            name: "Pikachu Yellow Cheeks",
            set: "Base Set",
            type_line: "Lightning / Basic",
            rarity: "Common",
            price: 18.50,
            stock: 24,
            accent: (247, 205, 69),
            description: "A bright entry card that makes the storefront feel approachable.",
        },
        Card {
            slug: "blastoise-shadowless",
            name: "Blastoise Shadowless",
            set: "Base Set",
            type_line: "Water / Stage 2",
            rarity: "Holo Rare",
            price: 189.00,
            stock: 5,
            accent: (69, 141, 240),
            description: "A premium water card with strong recognition and low stock pressure.",
        },
        Card {
            slug: "venusaur-holo",
            name: "Venusaur Holo",
            set: "Base Set",
            type_line: "Grass / Stage 2",
            rarity: "Holo Rare",
            price: 142.00,
            stock: 6,
            accent: (65, 178, 104),
            description: "A classic starter evolution card for complete-set buyers.",
        },
        Card {
            slug: "mewtwo-promo",
            name: "Mewtwo Movie Promo",
            set: "Black Star Promo",
            type_line: "Psychic / Basic",
            rarity: "Promo",
            price: 34.00,
            stock: 18,
            accent: (149, 104, 218),
            description: "A recognizable promo card that demonstrates mixed inventory tiers.",
        },
        Card {
            slug: "snorlax-jungle",
            name: "Snorlax Jungle",
            set: "Jungle",
            type_line: "Colorless / Basic",
            rarity: "Rare",
            price: 41.00,
            stock: 11,
            accent: (83, 125, 143),
            description: "A slower-moving collector favourite with a strong visual shape.",
        },
    ]
}

pub fn catalog_response() -> CatalogResponse {
    CatalogResponse {
        cards: cards()
            .iter()
            .map(|card| CardSummary {
                slug: card.slug.to_string(),
                name: card.name.to_string(),
                price: card.price,
                stock: card.stock,
            })
            .collect(),
    }
}
