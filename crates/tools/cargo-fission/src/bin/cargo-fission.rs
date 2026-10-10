fn main() -> anyhow::Result<()> {
    match fission_cli::run_from_env() {
        Err(error)
            if error
                .downcast_ref::<fission_command_run::review::ReviewExit>()
                .is_some() =>
        {
            let code = error
                .downcast_ref::<fission_command_run::review::ReviewExit>()
                .unwrap()
                .0;
            std::process::exit(code);
        }
        result => result,
    }
}
