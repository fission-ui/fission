mod app;
mod chrome;
mod pages;
mod routes;
mod state;
mod style;
mod ui;
mod widgets;

fn main() -> anyhow::Result<()> {
    app::run()
}
