use super::title_block;
use crate::actions::{
    request_command, set_host, set_port, toggle_detach, toggle_headless, toggle_no_open,
    toggle_release, RequestCommand, SetHost, SetPort, ToggleDetach, ToggleHeadless, ToggleNoOpen,
    ToggleRelease,
};
use crate::commands::UiCommand;
use crate::components::{
    ActionButton, ButtonTone, DeviceTable, FormTextField, KeyValueRow, TargetPicker, TogglePill,
};
use crate::state::{UiDevice, UiState};
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct RunScreen;

#[derive(Clone)]
pub struct BuildScreen;

#[derive(Clone)]
pub struct TestScreen;

impl From<RunScreen> for Widget {
    fn from(_component: RunScreen) -> Self {
        let (_ctx, _view) = fission::build::current::<UiState>();
        ExecutionScreen {
            title: "Run",
            description: "Launch the selected target on the selected device and attach output unless detach is enabled.",
            command: UiCommand::RunSelected,
            primary_label: "Run selected target",
            show_devices: true,
            show_host_port: true,
            show_detach: true,
            show_no_open: true,
            show_headless: true,
        }
        .into()
    }
}
impl From<BuildScreen> for Widget {
    fn from(_component: BuildScreen) -> Self {
        let (_ctx, _view) = fission::build::current::<UiState>();
        ExecutionScreen {
            title: "Build",
            description: "Build the selected target without launching it.",
            command: UiCommand::BuildSelected,
            primary_label: "Build selected target",
            show_devices: false,
            show_host_port: false,
            show_detach: false,
            show_no_open: false,
            show_headless: false,
        }
        .into()
    }
}
impl From<TestScreen> for Widget {
    fn from(_component: TestScreen) -> Self {
        let (_ctx, _view) = fission::build::current::<UiState>();
        ExecutionScreen {
            title: "Test",
            description: "Run the generated smoke test for the selected target.",
            command: UiCommand::TestSelected,
            primary_label: "Test selected target",
            show_devices: false,
            show_host_port: false,
            show_detach: false,
            show_no_open: false,
            show_headless: true,
        }
        .into()
    }
}
#[derive(Clone)]
struct ExecutionScreen {
    title: &'static str,
    description: &'static str,
    command: UiCommand,
    primary_label: &'static str,
    show_devices: bool,
    show_host_port: bool,
    show_detach: bool,
    show_no_open: bool,
    show_headless: bool,
}

impl From<ExecutionScreen> for Widget {
    fn from(component: ExecutionScreen) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let action = with_reducer!(
            ctx,
            RequestCommand(component.command.clone()),
            request_command
        );
        let mut sections = vec![
            title_block(
                component.title,
                component.description,
                palette.accent,
                palette.muted,
            ),
            Row {
                gap: Some(2.0),
                children: vec![
                    KeyValueRow::new("Target", view.state().selected_target_label()).into(),
                    KeyValueRow::new("Device", view.state().selected_device_label()).into(),
                ],
                ..Default::default()
            }
            .into(),
            TargetPicker {
                configured_only: true,
            }
            .into(),
        ];

        if component.show_devices {
            sections.push(Text::new("Runnable devices").color(palette.accent).into());
            sections.push(
                DeviceTable {
                    devices: current_target_devices(view),
                    selectable: true,
                    max_rows: 7,
                }
                .into(),
            );
        }

        if component.show_host_port {
            sections.push(network_fields(ctx, view));
        }
        sections.push(option_toggles(&component, ctx, view));
        sections.push(
            ActionButton::new(component.primary_label, action)
                .tone(ButtonTone::Primary)
                .width(26.0)
                .into(),
        );

        Column {
            gap: Some(1.0),
            children: sections,
            ..Default::default()
        }
        .into()
    }
}
fn network_fields(ctx: BuildCtxHandle<UiState>, view: ViewHandle<UiState>) -> Widget {
    let host = with_reducer!(ctx, SetHost(String::new()), set_host);
    let port = with_reducer!(ctx, SetPort(String::new()), set_port);
    Row {
        gap: Some(1.0),
        children: vec![
            FormTextField::new(
                "cli_ui_run_host",
                "Host",
                view.state().host.clone(),
                "127.0.0.1",
                host,
            )
            .width(24.0)
            .into(),
            FormTextField::new(
                "cli_ui_run_port",
                "Port",
                view.state().port.clone(),
                "8123",
                port,
            )
            .width(12.0)
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn current_target_devices(view: ViewHandle<UiState>) -> Vec<UiDevice> {
    view.state()
        .target_devices()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
}

fn option_toggles(
    screen: &ExecutionScreen,
    ctx: BuildCtxHandle<UiState>,
    view: ViewHandle<UiState>,
) -> Widget {
    let mut toggles = Vec::new();
    let release = with_reducer!(ctx, ToggleRelease, toggle_release);
    toggles.push(TogglePill::new("Release", view.state().release, release).into());

    if screen.show_detach {
        let detach = with_reducer!(ctx, ToggleDetach, toggle_detach);
        toggles.push(TogglePill::new("Detach", view.state().detach, detach).into());
    }
    if screen.show_no_open {
        let no_open = with_reducer!(ctx, ToggleNoOpen, toggle_no_open);
        toggles.push(TogglePill::new("No open", view.state().no_open, no_open).into());
    }
    if screen.show_headless {
        let headless = with_reducer!(ctx, ToggleHeadless, toggle_headless);
        toggles.push(TogglePill::new("Headless", view.state().headless, headless).into());
    }

    Row {
        gap: Some(1.0),
        children: toggles,
        ..Default::default()
    }
    .into()
}
