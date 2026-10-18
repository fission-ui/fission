fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<(), _>::new(counter::CounterApp::new()).run()
}
