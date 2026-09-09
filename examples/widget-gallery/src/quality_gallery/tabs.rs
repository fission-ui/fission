use fission::prelude::*;

use crate::GalleryState;

use super::general_panel::QualityGeneralPanel;

#[fission_reducer(SelectQualityTab)]
fn select_quality_tab(state: &mut GalleryState, index: usize) {
    state.active_tab = index;
}

pub(super) struct QualityTabs {
    pub compact: bool,
}

impl From<QualityTabs> for Widget {
    fn from(tabs: QualityTabs) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let active_tab = view.state().active_tab;
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

        let general_action = with_reducer!(ctx, SelectQualityTab(0), select_quality_tab);
        let security_action = with_reducer!(ctx, SelectQualityTab(1), select_quality_tab);
        let notifications_action = with_reducer!(ctx, SelectQualityTab(2), select_quality_tab);
        let active_panel: Widget = match active_tab {
            0 => TabPanel::new(
                general_panel_id,
                general_trigger_id,
                QualityGeneralPanel {
                    compact: tabs.compact,
                },
            )
            .into(),
            1 => TabPanel::new(
                security_panel_id,
                security_trigger_id,
                QualityPendingPanel {
                    title: "Security settings",
                    description: "Security controls will appear in this panel.",
                },
            )
            .into(),
            2 => TabPanel::new(
                notifications_panel_id,
                notifications_trigger_id,
                QualityPendingPanel {
                    title: "Notification settings",
                    description: "Notification controls will appear in this panel.",
                },
            )
            .into(),
            _ => Spacer::default().into(),
        };

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
            active_panel,
        ))
        .id(root_id)
    }
}

struct QualityPendingPanel {
    title: &'static str,
    description: &'static str,
}

impl From<QualityPendingPanel> for Widget {
    fn from(panel: QualityPendingPanel) -> Self {
        Card {
            child: EmptyState {
                icon: None,
                title: panel.title.into(),
                description: Some(panel.description.into()),
                action: None,
            }
            .into(),
            pattern: CardPattern::Plain,
            interactive: false,
            selected: false,
        }
        .into()
    }
}
