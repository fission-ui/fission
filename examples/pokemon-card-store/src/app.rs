use crate::components::{card_grid::CardGrid, hero::Hero, shell::StoreShell};
use crate::data;
use fission::prelude::*;

#[derive(Debug, Default)]
pub struct StoreState;
impl AppState for StoreState {}

#[derive(Clone)]
pub struct StoreHomePage;

impl Widget<StoreState> for StoreHomePage {
    fn build(&self, ctx: &mut BuildCtx<StoreState>, view: &View<StoreState>) -> Node {
        StoreShell {
            child: Column {
                gap: Some(28.0),
                children: vec![
                    Hero.build(ctx, view),
                    CardGrid {
                        cards: data::cards().to_vec(),
                    }
                    .build(ctx, view),
                ],
                ..Default::default()
            }
            .into_node(),
        }
        .build(ctx, view)
    }
}
