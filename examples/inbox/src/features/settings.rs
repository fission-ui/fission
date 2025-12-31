use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler, ActionEnvelope, ActionId};
use fission_core::ui::{Text, Node};
use fission_core::op::Color;
use fission_widgets::{Modal, ModalAction, VStack, HStack, Select, NumberInput, FormControl, SegmentedControl};
use crate::model::{InboxState, ToggleSettings, SetLocale};
use fission_i18n::{Locale};
use std::sync::Arc;
use serde_json;

pub struct SettingsModal;

impl Widget<InboxState> for SettingsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let locale_id = ctx.bind(SetLocale(Locale("".into())), (|s: &mut InboxState, a: SetLocale, _| s.locale = a.0) as Handler<InboxState, SetLocale>).id;

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
                        Text::new("Language").into_node(),
                        SegmentedControl {
                            options: vec!["English".into(), "Español".into()],
                            selected_index: if view.state.locale.0 == "es-ES" { 1 } else { 0 },
                            on_change: Some(Arc::new(move |idx| {
                                let loc = if idx == 1 { "es-ES" } else { "en-US" };
                                ActionEnvelope {
                                    id: locale_id,
                                    payload: serde_json::to_vec(&SetLocale(Locale(loc.into()))).unwrap(),
                                }
                            })),
                        }.build(ctx, view),

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