use fission::prelude::*;

use crate::GalleryState;

use super::general_panel::QualityGeneralPanel;

#[fission_reducer(SelectQualityTab)]
fn select_quality_tab(state: &mut GalleryState, index: usize) {
    state.active_tab = index;
}

fission_component! {
    pub(super) QualityTabs<GalleryState> { compact: bool }
    |tabs, state, cx| {
        let active_tab = state.active_tab;
        let root_id = WidgetId::explicit(if tabs.compact {
            "quality-gallery.tabs.mobile"
        } else {
            "quality-gallery.tabs.desktop"
        });
        let list_id = WidgetId::derived(root_id.as_u128(), &[1]);
        let general_trigger_id = WidgetId::derived(root_id.as_u128(), &[2]);
        let general_panel_id = WidgetId::derived(root_id.as_u128(), &[3]);
        let security_trigger_id = WidgetId::derived(root_id.as_u128(), &[4]);
        let security_panel_id = WidgetId::derived(root_id.as_u128(), &[5]);
        let notifications_trigger_id = WidgetId::derived(root_id.as_u128(), &[6]);
        let notifications_panel_id = WidgetId::derived(root_id.as_u128(), &[7]);

        let general_action = with_reducer!(cx.build, SelectQualityTab(0), select_quality_tab);
        let security_action = with_reducer!(cx.build, SelectQualityTab(1), select_quality_tab);
        let notifications_action = with_reducer!(cx.build, SelectQualityTab(2), select_quality_tab);
    } =>
        Widget::from(TabsLayout::new(
            TabList::new(
                list_id,
                vec![
                    TabTrigger::new(general_trigger_id, general_panel_id, "General")
                        .selected(active_tab == 0)
                        .on_press(general_action)
                        .size(ComponentSize::Md)
                        .semantics_identifier("quality-gallery.tabs.general"),
                    TabTrigger::new(security_trigger_id, security_panel_id, "Security")
                        .selected(active_tab == 1)
                        .on_press(security_action)
                        .size(ComponentSize::Md)
                        .semantics_identifier("quality-gallery.tabs.security"),
                    TabTrigger::new(
                        notifications_trigger_id,
                        notifications_panel_id,
                        "Notifications",
                    )
                    .selected(active_tab == 2)
                    .on_press(notifications_action)
                    .size(ComponentSize::Md)
                    .semantics_identifier("quality-gallery.tabs.notifications"),
                ],
            ),
            QualityActivePanel::new(
                active_tab,
                tabs.compact,
                general_panel_id,
                general_trigger_id,
                security_panel_id,
                security_trigger_id,
                notifications_panel_id,
                notifications_trigger_id,
            ),
        ))
        .id(root_id)
}

fission_component! {
    QualityActivePanel<GalleryState> {
        active_tab: usize,
        compact: bool,
        general_panel_id: WidgetId,
        general_trigger_id: WidgetId,
        security_panel_id: WidgetId,
        security_trigger_id: WidgetId,
        notifications_panel_id: WidgetId,
        notifications_trigger_id: WidgetId,
    } |panel, _state, _cx| {} => match panel.active_tab {
        0 => Widget::from(TabPanel::new(panel.general_panel_id, panel.general_trigger_id, QualityGeneralPanel::new(panel.compact))),
        1 => Widget::from(TabPanel::new(panel.security_panel_id, panel.security_trigger_id, QualityPendingPanel::new("Security settings", "Security controls will appear in this panel."))),
        2 => Widget::from(TabPanel::new(panel.notifications_panel_id, panel.notifications_trigger_id, QualityPendingPanel::new("Notification settings", "Notification controls will appear in this panel."))),
        _ => Widget::from(Spacer::default()),
    }
}

fission_component! {
    QualityPendingPanel<GalleryState> { title: &'static str, description: &'static str }
    |panel, _state, _cx| {} =>
        Card(
            child: EmptyState(
                icon: None,
                title: panel.title.to_string(),
                description: Some(panel.description.to_string()),
                action: None,
            ),
            pattern: CardPattern::Plain,
            interactive: false,
            selected: false,
        )
}
