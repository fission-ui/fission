use crate::gallery_switch_control::GallerySwitchControl;
use crate::state::GalleryState;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;
use fission::widgets::Slider;

fission_component! {
    #[derive(Clone)]
    pub GalleryControls<GalleryState> {
        toggle_smooth: ActionEnvelope,
        update_scale: ActionEnvelope,
        toggle_theme: ActionEnvelope,
        toggle_interactions: ActionEnvelope,
        toggle_animations: ActionEnvelope,
        toggle_markers: ActionEnvelope,
        instance: &'static str,
    }
    |controls, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            id: WidgetId::explicit(&format!("chart-gallery.controls.{}", controls.instance)),
            children: [
                Text("Chart controls").size(tokens.typography.label_large_size).color(tokens.colors.heading),
                Row(
                    children: [
                        GallerySwitchControl("Dark theme", state.dark_theme, controls.toggle_theme, format!("chart-gallery.control.{}.theme", controls.instance)),
                        GallerySwitchControl("Smooth lines", state.smooth, controls.toggle_smooth, format!("chart-gallery.control.{}.smooth", controls.instance)),
                        GallerySwitchControl("Interactions", state.interactions, controls.toggle_interactions, format!("chart-gallery.control.{}.interactions", controls.instance)),
                        GallerySwitchControl("Animations", state.animations, controls.toggle_animations, format!("chart-gallery.control.{}.animations", controls.instance)),
                        GallerySwitchControl("Markers", state.markers, controls.toggle_markers, format!("chart-gallery.control.{}.markers", controls.instance)),
                    ],
                    gap: tokens.spacing.m,
                    align_items: AlignItems::Center,
                    wrap: FlexWrap::Wrap,
                ),
                Row(
                    children: [
                        Text("Data scale").color(tokens.colors.text_primary),
                        Slider(
                            id: WidgetId::explicit(&format!("chart-gallery.control.{}.scale", controls.instance)),
                            semantics_identifier: format!("chart-gallery.control.{}.scale", controls.instance),
                            value: state.data_scale,
                            min: 0.1,
                            max: 2.0,
                            on_change: controls.update_scale.clone(),
                        ),
                        Text(format!("{:.2}x", state.data_scale)).color(tokens.colors.text_secondary),
                    ],
                    gap: tokens.spacing.m,
                    align_items: AlignItems::Center,
                ),
                Text(state.last_interaction.as_deref().unwrap_or("Interact with the chart to see typed chart events here."))
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
            ],
            gap: tokens.spacing.s,
            flex_shrink: 0.0,
        )
}
