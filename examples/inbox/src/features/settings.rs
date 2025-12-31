use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler};
use fission_core::ui::{Text, Node};
use fission_core::op::Color;
use fission_widgets::{Modal, ModalAction, VStack, HStack, Select, NumberInput, FormControl};
use crate::model::{InboxState, ToggleSettings, SetDensity};

pub struct SettingsModal;

impl Widget<InboxState> for SettingsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Modal {
            id: WidgetNodeId::explicit("settings_modal"),
            title: "Settings".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleSettings, (|s, _, _| s.show_settings = false) as Handler<InboxState, ToggleSettings>)),
            width: Some(400.0),
            content: Box::new(
                VStack {
                    spacing: Some(16.0),
                    children: vec![
                        Text::new("Appearance").into_node(),
                        
                        FormControl {
                            id: None,
                            label: Some("Theme".into()),
                            required: false,
                            error: None,
                            helper: None,
                            child: Box::new(Select {
                                id: WidgetNodeId::explicit("theme_select"),
                                selected_label: Some(view.state.theme_mode.clone()),
                                placeholder: "Select Theme".into(),
                                is_open: false,
                                on_toggle: None,
                                items: vec![],
                                ..Default::default()
                            }.build(ctx, view)),
                        }.build(ctx, view),

                        FormControl {
                            id: None,
                            label: Some("Density (Rows)".into()),
                            required: false,
                            error: None,
                            helper: Some("Rows per page".into()),
                            child: Box::new(NumberInput {
                                id: None,
                                value: 50.0, // Mock value
                                min: Some(10.0),
                                max: Some(100.0),
                                step: 10.0,
                                // Mock actions
                                on_increment: None, 
                                on_decrement: None,
                                on_change: None,
                            }.build(ctx, view)),
                        }.build(ctx, view),
                        
                        Text::new("Note: Select/Number widgets need dedicated state wiring.").size(12.0).color(Color { r: 100, g: 100, b: 100, a: 255 }).into_node(),
                    ]
                }.into_node()
            ),
            actions: vec![
                ModalAction { label: "Close".into(), is_primary: true, on_press: Some(ctx.bind(ToggleSettings, (|s, _, _| s.show_settings = false) as Handler<InboxState, ToggleSettings>)) }
            ]
        }.build(ctx, view)
    }
}