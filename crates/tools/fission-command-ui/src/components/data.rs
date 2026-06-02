use super::{ActionButton, ButtonTone};
use crate::actions::{select_device, select_target, SelectDevice, SelectTarget};
use crate::density::UiDensity;
use crate::state::{all_targets, target_label, UiDevice, UiState};
use crate::theme::UiPalette;
use fission::op::AlignItems;
use fission::prelude::*;
use fission_command_core::Target;

#[derive(Clone)]
pub struct KeyValueRow {
    pub label: String,
    pub value: String,
}

impl KeyValueRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

impl From<KeyValueRow> for Widget {
    fn from(component: KeyValueRow) -> Self {
        let (_ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        Row {
            gap: Some(1.0),
            align_items: AlignItems::Center,
            children: vec![
                Text::new(format!("{}:", component.label))
                    .color(palette.muted)
                    .width(16.0)
                    .into(),
                Text::new(component.value.clone())
                    .color(palette.text)
                    .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
#[derive(Clone)]
pub struct TargetPicker {
    pub configured_only: bool,
}

impl From<TargetPicker> for Widget {
    fn from(component: TargetPicker) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let targets = if component.configured_only && !view.state().targets.is_empty() {
            view.state().targets.clone()
        } else {
            all_targets().to_vec()
        };
        let mut rows = vec![Text::new("Target")
            .color(UiPalette::for_mode(view.state().theme_mode).accent)
            .into()];
        for target in targets {
            rows.push(target_button(target, ctx, view));
        }
        Column {
            gap: Some(if view.state().compact_mode { 0.0 } else { 1.0 }),
            children: rows,
            ..Default::default()
        }
        .into()
    }
}
fn target_button(
    target: Target,
    ctx: BuildCtxHandle<UiState>,
    view: ViewHandle<UiState>,
) -> Widget {
    let selected = view.state().selected_target == Some(target);
    let action = with_reducer!(ctx, SelectTarget(target), select_target);
    ActionButton::new(target_label(target), action)
        .tone(if selected {
            ButtonTone::Primary
        } else {
            ButtonTone::Neutral
        })
        .width(18.0)
        .into()
}

#[derive(Clone)]
pub struct DeviceTable {
    pub devices: Vec<UiDevice>,
    pub selectable: bool,
    pub max_rows: usize,
}

impl From<DeviceTable> for Widget {
    fn from(component: DeviceTable) -> Self {
        let (ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        let mut rows = vec![Row {
            gap: Some(1.0),
            children: vec![
                Text::new("target").color(palette.muted).width(10.0).into(),
                Text::new("kind").color(palette.muted).width(15.0).into(),
                Text::new("status").color(palette.muted).width(12.0).into(),
                Text::new("name").color(palette.muted).into(),
            ],
            ..Default::default()
        }
        .into()];
        for device in component.devices.iter().take(component.max_rows) {
            rows.push(device_row(device, component.selectable, ctx, view));
        }
        if component.devices.is_empty() {
            rows.push(
                Text::new("No devices detected.")
                    .color(palette.warning)
                    .into(),
            );
        }
        Column {
            gap: Some(if view.state().compact_mode { 0.0 } else { 1.0 }),
            children: rows,
            ..Default::default()
        }
        .into()
    }
}
fn device_row(
    device: &UiDevice,
    selectable: bool,
    ctx: BuildCtxHandle<UiState>,
    view: ViewHandle<UiState>,
) -> Widget {
    let palette = UiPalette::for_mode(view.state().theme_mode);
    let status_color = if device.available {
        palette.success
    } else {
        palette.warning
    };
    let selected = view.state().selected_device.as_ref() == Some(&device.id);
    let row = Row {
        gap: Some(1.0),
        children: vec![
            Text::new(device.target.as_str())
                .color(palette.text)
                .width(10.0)
                .into(),
            Text::new(device.kind.clone())
                .color(palette.text)
                .width(15.0)
                .into(),
            Text::new(device.status.clone())
                .color(status_color)
                .width(12.0)
                .into(),
            Text::new(format!("{} ({})", device.name, device.id))
                .color(palette.text)
                .into(),
        ],
        ..Default::default()
    }
    .into();

    if selectable && device.available {
        let action = with_reducer!(ctx, SelectDevice(device.id.clone()), select_device);
        let density = UiDensity::new(view.state().compact_mode);
        Button {
            on_press: Some(action),
            height: Some(density.control_height()),
            padding: Some(density.control_padding()),
            background_fill: Some(fission::op::Fill::Solid(if selected {
                palette.subtle
            } else {
                palette.surface
            })),
            text_color: Some(palette.text),
            child: Some(row),
            ..Default::default()
        }
        .into()
    } else {
        row
    }
}
