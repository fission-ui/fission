use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use fission_core::ui::{Text, Node};
use fission_core::op::Color;
use fission_widgets::{Modal, ModalAction, VStack, HStack, Select};
use crate::model::{InboxState, ToggleSettings};

pub struct SettingsModal;

impl Widget<InboxState> for SettingsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Modal {
            id: WidgetNodeId::explicit("settings_modal"),
            title: "Settings".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleSettings, |s, _| s.show_settings = false)),
            width: Some(400.0),
            content: Box::new(
                VStack {
                    spacing: Some(16.0),
                    children: vec![
                        Text::new("Appearance").into_node(),
                        HStack {
                            spacing: Some(12.0),
                            children: vec![
                                Text::new("Theme").into_node(),
                                Select {
                                    id: WidgetNodeId::explicit("theme_select"),
                                    selected_label: Some(view.state.theme_mode.clone()),
                                    placeholder: "Select Theme".into(),
                                    is_open: false, // In a real app this would be driven by local state or a dedicated field
                                    on_toggle: None, // Need dedicated state for select open
                                    items: vec![],
                                    ..Default::default()
                                }.build(ctx, view)
                            ]
                        }.into_node(),
                        Text::new("Note: Select widget requires dedicated open state. For demo, we assume standard theme.").size(12.0).color(Color { r: 100, g: 100, b: 100, a: 255 }).into_node(),
                    ]
                }.into_node()
            ),
            actions: vec![
                ModalAction { label: "Close".into(), is_primary: true, on_press: Some(ctx.bind(ToggleSettings, |s, _| s.show_settings = false)) }
            ]
        }.build(ctx, view)
    }
}