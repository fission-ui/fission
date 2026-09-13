use super::palette::{INK, MUTED};
use super::GuidedAgentTimeline;
use fission::prelude::*;

pub(super) struct GuidedWorkflow;
impl From<GuidedWorkflow> for Widget {
    fn from(_workflow: GuidedWorkflow) -> Self {
        Container::new(Column {
            gap: Some(0.0),
            children: widgets![
                Text::new("Customer feedback triage")
                    .size(30.0)
                    .line_height(35.0)
                    .weight(700)
                    .color(INK),
                Text::new("Turn recurring feedback into weekly product priorities.")
                    .size(16.0)
                    .line_height(24.0)
                    .color(MUTED),
                Spacer {
                    height: Some(8.0),
                    ..Default::default()
                },
                GuidedTabs,
                Spacer {
                    height: Some(12.0),
                    ..Default::default()
                },
                Text::new("Read from top to bottom. Each teammate passes work to the next.")
                    .size(14.0)
                    .line_height(20.0)
                    .color(MUTED),
                Spacer {
                    height: Some(12.0),
                    ..Default::default()
                },
                GuidedAgentTimeline,
                Spacer {
                    height: Some(10.0),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Lg,
                    content: Some(
                        ButtonContent::new("Add another teammate")
                            .leading_icon(Icon::svg(material::content::add::regular())),
                    ),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding([29.0, 22.0, 22.0, 16.0])
        .min_width(720.0)
        .into()
    }
}

pub(super) struct GuidedTabs;
impl From<GuidedTabs> for Widget {
    fn from(_tabs: GuidedTabs) -> Self {
        let root_id = WidgetId::explicit("quality-gallery.guided.tabs");
        let list_id = WidgetId::derived(root_id.as_u128(), &[1]);
        Container::new(Column {
            gap: Some(0.0),
            children: widgets![
                Row {
                    gap: Some(12.0),
                    align_items: fission::op::AlignItems::Center,
                    children: widgets![
                        TabList::new(
                            list_id,
                            vec![
                                GuidedTabLabel::trigger(root_id, 0, "Workflow", None, true),
                                GuidedTabLabel::trigger(root_id, 1, "Agents", Some("3"), false),
                                GuidedTabLabel::trigger(root_id, 2, "Hooks", Some("1"), false),
                                GuidedTabLabel::trigger(root_id, 3, "Packs", Some("2"), false),
                            ],
                        )
                        .presentation(TabPresentation::Underline),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Md,
                            content: Some(
                                ButtonContent::new("Add agent")
                                    .leading_icon(Icon::svg(material::content::add::regular()),)
                            ),
                            ..Default::default()
                        },
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Md,
                            child: Some(
                                Icon::svg(material::navigation::more_horiz::regular())
                                    .size(18.0)
                                    .color(INK)
                                    .into(),
                            ),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Divider::default(),
            ],
            ..Default::default()
        })
        .height(50.0)
        .into()
    }
}

pub(super) struct GuidedTabLabel;
impl GuidedTabLabel {
    fn trigger(
        root_id: WidgetId,
        index: u32,
        label: &'static str,
        count: Option<&'static str>,
        selected: bool,
    ) -> TabTrigger {
        let trigger_id = WidgetId::derived(root_id.as_u128(), &[10 + index]);
        let panel_id = WidgetId::derived(root_id.as_u128(), &[20 + index]);
        let mut content = TabTriggerContent::new(label);
        if let Some(count) = count {
            content = content.trailing_count(count);
        }
        TabTrigger::new(trigger_id, panel_id, label)
            .content(content)
            .selected(selected)
            .size(ComponentSize::Md)
    }
}
