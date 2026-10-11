mod app;

fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<app::CounterState, _>::new(app::CounterApp {}).run()
}
