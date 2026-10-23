use super::palette::{INK, MUTED};
use super::GuidedAgentTimeline;
use fission::prelude::*;

fission_component! {
    pub(super) GuidedWorkflow<()> {}
    |_workflow, _state, _cx| {} => Container(Column(
            gap: 0.0_f32,
            children: [
                Text("Customer feedback triage")
                    .size(30.0)
                    .line_height(35.0)
                    .weight(700)
                    .color(INK),
                Text("Turn recurring feedback into weekly product priorities.")
                    .size(16.0)
                    .line_height(24.0)
                    .color(MUTED),
                Spacer(height: 8.0_f32),
                GuidedTabs(),
                Spacer(height: 12.0_f32),
                Text("Read from top to bottom. Each teammate passes work to the next.")
                    .size(14.0)
                    .line_height(20.0)
                    .color(MUTED),
                Spacer(height: 12.0_f32),
                GuidedAgentTimeline(),
                Spacer(height: 10.0_f32),
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Lg,
                    content: ButtonContent::new("Add another teammate")
                        .leading_icon(Icon::svg(material::content::add::regular())),
                ),
            ],
        ))
        .padding([29.0, 22.0, 22.0, 16.0])
        .min_width(720.0)
}

fission_component! {
    pub(super) GuidedTabs<()> {}
    |_tabs, _state, _cx| {
        let root_id = WidgetId::explicit("quality-gallery.guided.tabs");
        let list_id = WidgetId::derived(root_id.as_u128(), &[1]);
    } => Container(Column(
            gap: 0.0_f32,
            children: [
                Row(
                    gap: 12.0_f32,
                    align_items: fission::op::AlignItems::Center,
                    children: [
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
                        Spacer(flex_grow: 1.0_f32),
                        Button(
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Md,
                            content: ButtonContent::new("Add agent")
                                .leading_icon(Icon::svg(material::content::add::regular()),),
                        ),
                        Button(
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Md,
                            child: Some(
                                Icon::svg(material::navigation::more_horiz::regular())
                                    .size(18.0)
                                    .color(INK)
                                    .into()
                            ),
                        ),
                    ],
                ),
                Divider::default(),
            ],
        ))
        .height(50.0)
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
