//! Rendering only: execution belongs to the existing run/site authorities.
use anyhow::Result;
use fission_command_run::serving::ServingEvent;
use serde::Serialize;
use std::{
    ffi::OsString,
    io::{self, Write},
    path::Path,
};

#[derive(Serialize)]
struct Event<'a> {
    schema: &'static str,
    action: &'a str,
    project_dir: &'a Path,
    owner_pid: u32,
    event: ServingEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_argv: Option<Vec<String>>,
}

pub(crate) fn execute(
    action: &str,
    project_dir: &Path,
    json: bool,
    argv: &[OsString],
    work: impl FnOnce(&mut dyn FnMut(ServingEvent) -> Result<()>) -> Result<()>,
) -> Result<()> {
    let failed = std::cell::Cell::new(false);
    let mut render = |event| {
        failed.set(failed.get() || matches!(event, ServingEvent::Failed { .. }));
        if !json {
            return fission_command_site::serving::human_event(event);
        }
        let retry_argv = matches!(event, ServingEvent::Failed { .. }).then(|| {
            argv.iter()
                .map(|v| v.to_string_lossy().into_owned())
                .collect()
        });
        let report = Event {
            schema: "fission.cli-event.v1",
            action,
            project_dir,
            owner_pid: std::process::id(),
            event,
            retry_argv,
        };
        let mut stdout = io::stdout().lock();
        serde_json::to_writer(&mut stdout, &report)?;
        writeln!(stdout)?;
        stdout.flush()?;
        Ok(())
    };
    let result = work(&mut render);
    if let Err(ref error) = result {
        if !failed.get() {
            render(ServingEvent::Failed {
                message: format!("{error:#}"),
                owned_resources_released: true,
            })?;
        }
    }
    result
}
