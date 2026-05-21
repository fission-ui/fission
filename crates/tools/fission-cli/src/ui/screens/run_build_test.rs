use super::title_block;
use crate::ui::actions::{
    request_command, set_host, set_port, toggle_detach, toggle_headless, toggle_no_open,
    toggle_release, RequestCommand, SetHost, SetPort, ToggleDetach, ToggleHeadless, ToggleNoOpen,
    ToggleRelease,
};
use crate::ui::commands::UiCommand;
use crate::ui::components::{
    ActionButton, ButtonTone, DeviceTable, FormTextField, KeyValueRow, TargetPicker, TogglePill,
};
use crate::ui::state::{UiDevice, UiState};
use crate::ui::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct RunScreen;

#[derive(Clone)]
pub(crate) struct BuildScreen;

#[derive(Clone)]
pub(crate) struct TestScreen;

impl Widget<UiState> for RunScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
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
        .build(ctx, view)
    }
}

impl Widget<UiState> for BuildScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
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
        .build(ctx, view)
    }
}

impl Widget<UiState> for TestScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
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
        .build(ctx, view)
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

impl Widget<UiState> for ExecutionScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let action = with_reducer!(ctx, RequestCommand(self.command.clone()), request_command);
        let mut sections = vec![
            title_block(self.title, self.description, palette.accent, palette.muted),
            Row {
                gap: Some(2.0),
                children: vec![
                    KeyValueRow::new("Target", view.state.selected_target_label()).build(ctx, view),
                    KeyValueRow::new("Device", view.state.selected_device_label()).build(ctx, view),
                ],
                ..Default::default()
            }
            .into_node(),
            TargetPicker {
                configured_only: true,
            }
            .build(ctx, view),
        ];

        if self.show_devices {
            sections.push(
                Text::new("Runnable devices")
                    .color(palette.accent)
                    .into_node(),
            );
            sections.push(
                DeviceTable {
                    devices: current_target_devices(view),
                    selectable: true,
                    max_rows: 7,
                }
                .build(ctx, view),
            );
        }

        if self.show_host_port {
            sections.push(network_fields(ctx, view));
        }
        sections.push(option_toggles(self, ctx, view));
        sections.push(
            ActionButton::new(self.primary_label, action)
                .tone(ButtonTone::Primary)
                .width(26.0)
                .build(ctx, view),
        );

        Column {
            gap: Some(1.0),
            children: sections,
            ..Default::default()
        }
        .into_node()
    }
}

fn network_fields(ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
    let host = with_reducer!(ctx, SetHost(String::new()), set_host);
    let port = with_reducer!(ctx, SetPort(String::new()), set_port);
    Row {
        gap: Some(1.0),
        children: vec![
            FormTextField::new(
                "cli_ui_run_host",
                "Host",
                view.state.host.clone(),
                "127.0.0.1",
                host,
            )
            .width(24.0)
            .build(ctx, view),
            FormTextField::new(
                "cli_ui_run_port",
                "Port",
                view.state.port.clone(),
                "8123",
                port,
            )
            .width(12.0)
            .build(ctx, view),
        ],
        ..Default::default()
    }
    .into_node()
}

fn current_target_devices(view: &View<UiState>) -> Vec<UiDevice> {
    view.state
        .target_devices()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
}

fn option_toggles(
    screen: &ExecutionScreen,
    ctx: &mut BuildCtx<UiState>,
    view: &View<UiState>,
) -> Node {
    let mut toggles = Vec::new();
    let release = with_reducer!(ctx, ToggleRelease, toggle_release);
    toggles.push(TogglePill::new("Release", view.state.release, release).build(ctx, view));

    if screen.show_detach {
        let detach = with_reducer!(ctx, ToggleDetach, toggle_detach);
        toggles.push(TogglePill::new("Detach", view.state.detach, detach).build(ctx, view));
    }
    if screen.show_no_open {
        let no_open = with_reducer!(ctx, ToggleNoOpen, toggle_no_open);
        toggles.push(TogglePill::new("No open", view.state.no_open, no_open).build(ctx, view));
    }
    if screen.show_headless {
        let headless = with_reducer!(ctx, ToggleHeadless, toggle_headless);
        toggles.push(TogglePill::new("Headless", view.state.headless, headless).build(ctx, view));
    }

    Row {
        gap: Some(1.0),
        children: toggles,
        ..Default::default()
    }
    .into_node()
}
