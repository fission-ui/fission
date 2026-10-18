use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    Accordion, AccordionItem, CardContent, CardDescription, CardFooter, CardHeader, CardLayout,
    CardTitle, Stepper, Timeline, TimelineItem, TreeItem, TreeView,
};

const CARD_MAX_WIDTH: f32 = 420.0;

#[fission_reducer(ToggleAccordion)]
fn toggle_accordion(state: &mut GalleryState, index: usize) {
    state.accordion_open = if state.accordion_open == index {
        usize::MAX
    } else {
        index
    };
}

#[fission_reducer(ToggleTreeNode)]
fn toggle_tree_node(state: &mut GalleryState, id: String) {
    if !state.tree_expanded.remove(&id) {
        state.tree_expanded.insert(id);
    }
}

#[fission_reducer(SelectTreeNode)]
fn select_tree_node(state: &mut GalleryState, id: String) {
    state.tree_selected = Some(id);
}

pub(crate) fn card() -> Vec<Widget> {
    widgets![CardDemo::new()]
}

fission_component! {
    CardDemo<GalleryState> |_demo, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
    Container(
        CardLayout::new()
            .header(
                CardHeader::new(CardTitle::new("Messages synced"))
                    .description(CardDescription::new(
                        "Across every connected inbox this week.",
                    ))
                    .action(CardOptionsButton::new()),
            )
            .content(CardContent::new(
                Text::new("1,284")
                    .size(typography.heading2_size)
                    .weight(typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
            ))
            .footer(CardFooter::new(vec![
                CardFooterButton::new(ButtonVariant::Outline, "View report").into(),
                CardFooterButton::new(ButtonVariant::Filled, "Sync now").into(),
            ])),
    )
    .max_width_length(Length::points(CARD_MAX_WIDTH))
}

fission_component! {
    CardOptionsButton<()> |_button, _state, _cx| {} =>
    Button(
        variant: ButtonVariant::Ghost,
        icon_content: ButtonIconContent::new(
            Icon::svg(fission::icons::material::navigation::more_horiz::regular()),
            "Card options",
        ),
    )
}

fission_component! {
    CardFooterButton<()> { variant: ButtonVariant, label: &'static str }
    |button, _state, _cx| {} =>
    Button(
        variant: button.variant,
        size: ComponentSize::Sm,
        child: Text(button.label),
    )
}

pub(crate) fn accordion() -> Vec<Widget> {
    widgets![AccordionDemo::new()]
}

fission_component! {
    AccordionDemo<GalleryState> |_demo, state, cx| {} =>
    Accordion(
        items: vec![
            AccordionItem {
                title: "Section 1".into(),
                content: Text::new("Content of section 1").into(),
                is_expanded: state.accordion_open == 0,
                on_toggle: Some(with_reducer!(cx.build, ToggleAccordion(0), toggle_accordion)),
            },
            AccordionItem {
                title: "Section 2".into(),
                content: Text::new("Content of section 2").into(),
                is_expanded: state.accordion_open == 1,
                on_toggle: Some(with_reducer!(cx.build, ToggleAccordion(1), toggle_accordion)),
            },
        ],
        motion: None,
    )
}

pub(crate) fn stepper() -> Vec<Widget> {
    widgets![StepperDemo::new()]
}

fission_component! {
    StepperDemo<()> |_demo, _state, _cx| {} =>
    Stepper(
        steps: vec![
            "Import".into(),
            "Configure".into(),
            "Review".into(),
            "Deploy".into(),
        ],
        active_index: 1_usize,
    )
}

pub(crate) fn timeline() -> Vec<Widget> {
    widgets![TimelineDemo::new()]
}

fission_component! {
    TimelineDemo<()> |_demo, _state, _cx| {} =>
    Timeline(
        items: vec![
            TimelineItem {
                title: "Created".into(),
                description: Some("Project initialized".into()),
                timestamp: Some("2025-01-01".into()),
            },
            TimelineItem {
                title: "Updated".into(),
                description: Some("Added widgets".into()),
                timestamp: Some("2025-02-15".into()),
            },
            TimelineItem {
                title: "Released".into(),
                description: None,
                timestamp: Some("2025-03-01".into()),
            },
        ],
    )
}

pub(crate) fn tree_view() -> Vec<Widget> {
    widgets![TreeViewDemo::new()]
}

fission_component! {
    TreeViewDemo<GalleryState> |_demo, state, cx| {} =>
    TreeView(
        items: vec![TreeItem {
            id: "src".into(),
            label: "src/".into(),
            icon: None,
            children: vec![
                TreeItem {
                    id: "main".into(),
                    label: "main.rs".into(),
                    icon: None,
                    children: vec![],
                    on_toggle: None,
                    on_select: Some(with_reducer!(
                        cx.build,
                        SelectTreeNode("main".into()),
                        select_tree_node
                    )),
                },
                TreeItem {
                    id: "lib".into(),
                    label: "lib.rs".into(),
                    icon: None,
                    children: vec![],
                    on_toggle: None,
                    on_select: Some(with_reducer!(
                        cx.build,
                        SelectTreeNode("lib".into()),
                        select_tree_node
                    )),
                },
            ],
            on_toggle: Some(with_reducer!(
                cx.build,
                ToggleTreeNode("src".into()),
                toggle_tree_node
            )),
            on_select: None,
        }],
        expanded_ids: state.tree_expanded.clone(),
        selected_id: state.tree_selected.clone(),
    )
}
