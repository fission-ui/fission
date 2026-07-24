use crate::gallery_switch_control::GallerySwitchControl;
use crate::state::GalleryState;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;

#[derive(Clone)]
pub struct GalleryControls {
    pub toggle_smooth: ActionEnvelope,
    pub update_scale: ActionEnvelope,
    pub toggle_theme: ActionEnvelope,
    pub toggle_interactions: ActionEnvelope,
    pub toggle_animations: ActionEnvelope,
    pub toggle_markers: ActionEnvelope,
    pub instance: &'static str,
}

impl From<GalleryControls> for Widget {
    fn from(controls: GalleryControls) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;

        Column {
            id: Some(WidgetId::explicit(&format!(
                "chart-gallery.controls.{}",
                controls.instance
            ))),
            children: widgets![
                Text::new("Chart controls")
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.heading),
                Row {
                    children: widgets![
                        GallerySwitchControl {
                            label: "Dark theme",
                            checked: view.state().dark_theme,
                            action: controls.toggle_theme,
                            identifier: format!(
                                "chart-gallery.control.{}.theme",
                                controls.instance
                            ),
                        },
                        GallerySwitchControl {
                            label: "Smooth lines",
                            checked: view.state().smooth,
                            action: controls.toggle_smooth,
                            identifier: format!(
                                "chart-gallery.control.{}.smooth",
                                controls.instance
                            ),
                        },
                        GallerySwitchControl {
                            label: "Interactions",
                            checked: view.state().interactions,
                            action: controls.toggle_interactions,
                            identifier: format!(
                                "chart-gallery.control.{}.interactions",
                                controls.instance
                            ),
                        },
                        GallerySwitchControl {
                            label: "Animations",
                            checked: view.state().animations,
                            action: controls.toggle_animations,
                            identifier: format!(
                                "chart-gallery.control.{}.animations",
                                controls.instance
                            ),
                        },
                        GallerySwitchControl {
                            label: "Markers",
                            checked: view.state().markers,
                            action: controls.toggle_markers,
                            identifier: format!(
                                "chart-gallery.control.{}.markers",
                                controls.instance
                            ),
                        },
                    ],
                    gap: Some(tokens.spacing.m),
                    align_items: AlignItems::Center,
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                },
                Row {
                    children: widgets![
                        Text::new("Data scale").color(tokens.colors.text_primary),
                        fission::widgets::Slider {
                            id: Some(WidgetId::explicit(&format!(
                                "chart-gallery.control.{}.scale",
                                controls.instance
                            ))),
                            semantics_identifier: Some(format!(
                                "chart-gallery.control.{}.scale",
                                controls.instance
                            )),
                            value: view.state().data_scale,
                            min: 0.1,
                            max: 2.0,
                            on_change: Some(controls.update_scale),
                            ..Default::default()
                        },
                        Text::new(format!("{:.2}x", view.state().data_scale))
                            .color(tokens.colors.text_secondary),
                    ],
                    gap: Some(tokens.spacing.m),
                    align_items: AlignItems::Center,
                    ..Default::default()
                },
                Text::new(
                    view.state()
                        .last_interaction
                        .as_deref()
                        .unwrap_or("Interact with the chart to see typed chart events here."),
                )
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        }
        .into()
    }
}
