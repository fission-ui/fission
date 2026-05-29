use fission::server::run_from_cli;
use pokemon_card_store::pokemon_card_store_server;

fn main() -> anyhow::Result<()> {
    run_from_cli(pokemon_card_store_server())
}
