use crate::gallery_section::GallerySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    Accordion, AccordionItem, Card, Stepper, Timeline, TimelineItem, TreeItem, TreeView, VStack,
};

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

pub(crate) struct DataSection;

impl From<DataSection> for Widget {
    fn from(_section: DataSection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        GallerySection::new(
            "Data Display",
            widgets![
                Card {
                    child: VStack {
                        spacing: Some(tokens.spacing.xs),
                        children: widgets![
                            Text::new("Card Title")
                                .size(typography.font_size_lg)
                                .weight(typography.font_weight_bold)
                                .color(tokens.colors.text_primary),
                            Text::new("Some card content goes here.")
                                .color(tokens.colors.text_secondary),
                        ],
                    }
                    .into(),
                    ..Default::default()
                },
                Accordion {
                    items: vec![
                        AccordionItem {
                            title: "Section 1".into(),
                            content: Text::new("Content of section 1").into(),
                            is_expanded: state.accordion_open == 0,
                            on_toggle: Some(with_reducer!(
                                ctx,
                                ToggleAccordion(0),
                                toggle_accordion
                            )),
                        },
                        AccordionItem {
                            title: "Section 2".into(),
                            content: Text::new("Content of section 2").into(),
                            is_expanded: state.accordion_open == 1,
                            on_toggle: Some(with_reducer!(
                                ctx,
                                ToggleAccordion(1),
                                toggle_accordion
                            )),
                        },
                    ],
                    motion: None,
                },
                Stepper {
                    steps: vec![
                        "Import".into(),
                        "Configure".into(),
                        "Review".into(),
                        "Deploy".into(),
                    ],
                    active_index: 1,
                },
                Timeline {
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
                },
                TreeView {
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
                                    ctx,
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
                                    ctx,
                                    SelectTreeNode("lib".into()),
                                    select_tree_node
                                )),
                            },
                        ],
                        on_toggle: Some(with_reducer!(
                            ctx,
                            ToggleTreeNode("src".into()),
                            toggle_tree_node
                        )),
                        on_select: None,
                    }],
                    expanded_ids: state.tree_expanded.clone(),
                    selected_id: state.tree_selected.clone(),
                },
            ],
        )
        .into()
    }
}
