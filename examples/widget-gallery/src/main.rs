use fission_core::op::Color as IrColor;
use fission_core::ui::{
    Button, ButtonVariant, Checkbox, Column, Container, Node, Row, Scroll, Slider, Switch, Text,
    TextContent, TextInput,
};
use fission_core::{
    ActionEnvelope, AppState, BuildCtx, FlexDirection, Handler, ReducerContext, View, Widget,
    WidgetNodeId,
};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use fission_widgets::{
    Accordion, AccordionItem, Alert, AlertKind, Avatar, Badge, Breadcrumb, BreadcrumbItem, Card,
    CircularProgress, Code, Divider, Drawer, DrawerSide, EmptyState, HStack,
    Kbd, Link, MenuButton, MenuItem, Modal, ModalAction, NumberInput, Pagination,
    ProgressBar, SegmentedControl, Select, SelectItem, Skeleton,
    Spacer, Spinner, Stat, Stepper, TabItem, Tabs, Tag, Timeline,
    TimelineItem, Toast, ToastKind, Tooltip, TreeItem, TreeView, VStack,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

// --- State ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GalleryState {
    slider_val: f32,
    range_start: f32,
    range_end: f32,
    checked: bool,
    switch_on: bool,
    radio_choice: usize,
    text_value: String,
    number_val: f32,
    active_tab: usize,
    accordion_open: usize,
    select_open: bool,
    select_value: Option<String>,
    menu_open: bool,
    modal_open: bool,
    drawer_open: bool,
    tooltip_vis: bool,
    popover_open: bool,
    segmented_idx: usize,
    current_page: usize,
    tree_expanded: HashSet<String>,
    tree_selected: Option<String>,
    show_toast: bool,
}

impl Default for GalleryState {
    fn default() -> Self {
        let mut tree_expanded = HashSet::new();
        tree_expanded.insert("src".into());
        Self {
            slider_val: 50.0,
            range_start: 0.0,
            range_end: 0.0,
            checked: true,
            switch_on: true,
            radio_choice: 0,
            text_value: String::new(),
            number_val: 5.0,
            active_tab: 0,
            accordion_open: 0,
            select_open: false,
            select_value: None,
            menu_open: false,
            modal_open: false,
            drawer_open: false,
            tooltip_vis: false,
            popover_open: false,
            segmented_idx: 0,
            current_page: 1,
            tree_expanded,
            tree_selected: None,
            show_toast: false,
        }
    }
}

impl AppState for GalleryState {}

// --- Actions ---

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
struct SetSlider(f32);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleChecked;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleSwitch;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SetRadio(usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
struct UpdateText(String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq)]
struct IncrementNumber;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq)]
struct DecrementNumber;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SetTab(usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleAccordion(usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleSelect;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SelectValue(String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleMenu;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleModal;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleDrawer;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct TogglePopover;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SetSegmented(usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SetPage(usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleTreeNode(String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SelectTreeNode(String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct DismissToast;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ShowToast;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Noop;

// --- Helpers ---

fn section(title: &str, children: Vec<Node>) -> Node {
    VStack {
        spacing: Some(8.0),
        children: vec![
            vec![
                Spacer {
                    height: Some(8.0),
                    ..Default::default()
                }
                .into_node(),
                Text::new(title).size(20.0).into_node(),
                Divider::default().build_inline(),
            ],
            children,
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
    .into_node()
}

trait BuildInline {
    fn build_inline(self) -> Node;
}

impl BuildInline for Divider {
    fn build_inline(self) -> Node {
        Container::new(
            fission_core::ui::widgets::Spacer::default().into_node(),
        )
        .height(1.0)
        .bg(IrColor {
            r: 200,
            g: 200,
            b: 200,
            a: 255,
        })
        .flex_grow(1.0)
        .into_node()
    }
}

// --- App Widget ---

struct GalleryApp;

impl Widget<GalleryState> for GalleryApp {
    fn build(&self, ctx: &mut BuildCtx<GalleryState>, view: &View<GalleryState>) -> Node {
        let s = view.state;
        let tokens = &view.env.theme.tokens;

        // -- Display Widgets --
        let display_section = section(
            "Display",
            vec![
                HStack {
                    spacing: Some(12.0),
                    children: vec![
                        Text::new("Hello Fission").size(16.0).into_node(),
                        Badge {
                            text: "New".into(),
                            ..Default::default()
                        }
                        .build(ctx, view),
                        Tag {
                            label: "Rust".into(),
                            on_close: None,
                        }
                        .build(ctx, view),
                        Avatar {
                            name: Some("John Doe".into()),
                            src: None,
                            size: Some(36.0),
                        }
                        .build(ctx, view),
                    ],
                }
                .into_node(),
                HStack {
                    spacing: Some(12.0),
                    children: vec![
                        Code {
                            text: "let x = 42;".into(),
                        }
                        .build(ctx, view),
                        Kbd {
                            text: "Ctrl+C".into(),
                        }
                        .build(ctx, view),
                    ],
                }
                .into_node(),
                Stat {
                    label: "Total Users".into(),
                    value: "1,234".into(),
                    help_text: Some("+12% this month".into()),
                }
                .build(ctx, view),
            ],
        );

        // -- Input Widgets --
        let input_section = section(
            "Input",
            vec![
                // Button variants
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Box::new(Text::new("Filled").into_node())),
                            on_press: Some(ctx.bind(
                                Noop,
                                (|_, _: Noop, _| {}) as Handler<GalleryState, Noop>,
                            )),
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Box::new(Text::new("Outline").into_node())),
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Box::new(Text::new("Ghost").into_node())),
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Box::new(Text::new("Disabled").into_node())),
                            disabled: true,
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                }
                .into_node(),
                // TextInput
                TextInput {
                    value: s.text_value.clone(),
                    placeholder: Some("Type something...".into()),
                    on_change: Some(ctx.bind(
                        UpdateText(String::new()),
                        (|s: &mut GalleryState, a: UpdateText, _| s.text_value = a.0)
                            as Handler<GalleryState, UpdateText>,
                    )),
                    width: Some(300.0),
                    ..Default::default()
                }
                .into_node(),
                // Checkbox + Switch + Radio
                HStack {
                    spacing: Some(16.0),
                    children: vec![
                        Checkbox {
                            checked: s.checked,
                            on_toggle: Some(ctx.bind(
                                ToggleChecked,
                                (|s: &mut GalleryState, _, _| s.checked = !s.checked)
                                    as Handler<GalleryState, ToggleChecked>,
                            )),
                            label: Some("Check me".into()),
                            ..Default::default()
                        }
                        .into_node(),
                        Switch {
                            checked: s.switch_on,
                            on_toggle: Some(ctx.bind(
                                ToggleSwitch,
                                (|s: &mut GalleryState, _, _| s.switch_on = !s.switch_on)
                                    as Handler<GalleryState, ToggleSwitch>,
                            )),
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                }
                .into_node(),
                // Slider
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Text::new("Slider:").into_node(),
                        Container::new(
                            Slider {
                                value: s.slider_val,
                                min: 0.0,
                                max: 100.0,
                                on_change: Some(ctx.bind(
                                    SetSlider(0.0),
                                    (|s: &mut GalleryState, a: SetSlider, _| s.slider_val = a.0)
                                        as Handler<GalleryState, SetSlider>,
                                )),
                                ..Default::default()
                            }
                            .into_node(),
                        )
                        .width(200.0)
                        .into_node(),
                        Text::new(format!("{:.0}", s.slider_val)).into_node(),
                    ],
                }
                .into_node(),
                // NumberInput
                NumberInput {
                    value: s.number_val,
                    step: 1.0,
                    on_increment: Some(ctx.bind(
                        IncrementNumber,
                        (|s: &mut GalleryState, _, _| s.number_val += 1.0)
                            as Handler<GalleryState, IncrementNumber>,
                    )),
                    on_decrement: Some(ctx.bind(
                        DecrementNumber,
                        (|s: &mut GalleryState, _, _| s.number_val -= 1.0)
                            as Handler<GalleryState, DecrementNumber>,
                    )),
                    ..Default::default()
                }
                .build(ctx, view),
            ],
        );

        // -- Feedback Widgets --
        let feedback_section = section(
            "Feedback",
            vec![
                Alert {
                    kind: AlertKind::Info,
                    title: "Information".into(),
                    description: Some("This is an info alert.".into()),
                }
                .build(ctx, view),
                Alert {
                    kind: AlertKind::Success,
                    title: "Success".into(),
                    description: None,
                }
                .build(ctx, view),
                Alert {
                    kind: AlertKind::Warning,
                    title: "Warning".into(),
                    description: Some("Be careful!".into()),
                }
                .build(ctx, view),
                Alert {
                    kind: AlertKind::Error,
                    title: "Error".into(),
                    description: Some("Something went wrong.".into()),
                }
                .build(ctx, view),
                HStack {
                    spacing: Some(16.0),
                    children: vec![
                        ProgressBar { value: 0.65 }.build(ctx, view),
                    ],
                }
                .into_node(),
                HStack {
                    spacing: Some(16.0),
                    children: vec![
                        Spinner {
                            id: WidgetNodeId::explicit("spinner1"),
                            color: None,
                        }
                        .build(ctx, view),
                        CircularProgress {
                            value: Some(0.7),
                            size: 40.0,
                            ..Default::default()
                        }
                        .build(ctx, view),
                        Skeleton {
                            id: WidgetNodeId::explicit("skel1"),
                            width: Some(120.0),
                            height: Some(20.0),
                            circle: false,
                        }
                        .build(ctx, view),
                    ],
                }
                .into_node(),
                EmptyState {
                    icon: None,
                    title: "No items yet".into(),
                    description: Some("Add your first item to get started.".into()),
                    action: Some(Box::new(
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Box::new(Text::new("Add Item").into_node())),
                            ..Default::default()
                        }
                        .into_node(),
                    )),
                }
                .build(ctx, view),
            ],
        );

        // -- Navigation Widgets --
        let nav_section = section(
            "Navigation",
            vec![
                // Tabs
                Tabs {
                    active_index: s.active_tab,
                    items: vec![
                        TabItem {
                            title: "Tab A".into(),
                            content: Text::new("Content of Tab A").into_node(),
                            on_press: Some(ctx.bind(
                                SetTab(0),
                                (|s: &mut GalleryState, a: SetTab, _| s.active_tab = a.0)
                                    as Handler<GalleryState, SetTab>,
                            )),
                        },
                        TabItem {
                            title: "Tab B".into(),
                            content: Text::new("Content of Tab B").into_node(),
                            on_press: Some(ctx.bind(
                                SetTab(1),
                                (|s: &mut GalleryState, a: SetTab, _| s.active_tab = a.0)
                                    as Handler<GalleryState, SetTab>,
                            )),
                        },
                        TabItem {
                            title: "Tab C".into(),
                            content: Text::new("Content of Tab C").into_node(),
                            on_press: Some(ctx.bind(
                                SetTab(2),
                                (|s: &mut GalleryState, a: SetTab, _| s.active_tab = a.0)
                                    as Handler<GalleryState, SetTab>,
                            )),
                        },
                    ],
                }
                .build(ctx, view),
                // Breadcrumb
                Breadcrumb {
                    items: vec![
                        BreadcrumbItem {
                            label: "Home".into(),
                            on_click: None,
                        },
                        BreadcrumbItem {
                            label: "Gallery".into(),
                            on_click: None,
                        },
                        BreadcrumbItem {
                            label: "Widgets".into(),
                            on_click: None,
                        },
                    ],
                }
                .build(ctx, view),
                // SegmentedControl
                SegmentedControl {
                    options: vec!["Day".into(), "Week".into(), "Month".into()],
                    selected_index: s.segmented_idx,
                    on_change: Some(Arc::new({
                        let env = ctx.bind(
                            SetSegmented(0),
                            (|s: &mut GalleryState, a: SetSegmented, _| s.segmented_idx = a.0)
                                as Handler<GalleryState, SetSegmented>,
                        );
                        move |idx| {
                            ActionEnvelope {
                                id: env.id,
                                payload: serde_json::to_vec(&idx).unwrap(),
                            }
                        }
                    })),
                }
                .build(ctx, view),
                // Pagination
                Pagination {
                    current_page: s.current_page.max(1),
                    total_pages: 10,
                    on_change: Some(Arc::new({
                        let env = ctx.bind(
                            SetPage(1),
                            (|s: &mut GalleryState, a: SetPage, _| s.current_page = a.0)
                                as Handler<GalleryState, SetPage>,
                        );
                        move |page| {
                            ActionEnvelope {
                                id: env.id,
                                payload: serde_json::to_vec(&page).unwrap(),
                            }
                        }
                    })),
                }
                .build(ctx, view),
                // Link
                Link {
                    text: "Visit documentation".into(),
                    on_click: None,
                }
                .build(ctx, view),
                // MenuButton
                MenuButton {
                    id: WidgetNodeId::explicit("gallery_menu"),
                    label: "Actions".into(),
                    items: vec![
                        MenuItem {
                            label: "Edit".into(),
                            icon: None,
                            on_select: None,
                        },
                        MenuItem {
                            label: "Delete".into(),
                            icon: None,
                            on_select: None,
                        },
                    ],
                    is_open: s.menu_open,
                    on_toggle: Some(ctx.bind(
                        ToggleMenu,
                        (|s: &mut GalleryState, _, _| s.menu_open = !s.menu_open)
                            as Handler<GalleryState, ToggleMenu>,
                    )),
                }
                .build(ctx, view),
            ],
        );

        // -- Data Widgets --
        let data_section = section(
            "Data Display",
            vec![
                // Card
                Card {
                    child: Box::new(
                        VStack {
                            spacing: Some(4.0),
                            children: vec![
                                Text::new("Card Title").size(18.0).into_node(),
                                Text::new("Some card content goes here.")
                                    .color(tokens.colors.text_secondary)
                                    .into_node(),
                            ],
                        }
                        .into_node(),
                    ),
                }
                .build(ctx, view),
                // Accordion
                Accordion {
                    items: vec![
                        AccordionItem {
                            title: "Section 1".into(),
                            content: Text::new("Content of section 1").into_node(),
                            is_expanded: s.accordion_open == 0,
                            on_toggle: Some(ctx.bind(
                                ToggleAccordion(0),
                                (|s: &mut GalleryState, a: ToggleAccordion, _| {
                                    s.accordion_open = if s.accordion_open == a.0 {
                                        usize::MAX
                                    } else {
                                        a.0
                                    }
                                })
                                    as Handler<GalleryState, ToggleAccordion>,
                            )),
                        },
                        AccordionItem {
                            title: "Section 2".into(),
                            content: Text::new("Content of section 2").into_node(),
                            is_expanded: s.accordion_open == 1,
                            on_toggle: Some(ctx.bind(
                                ToggleAccordion(1),
                                (|s: &mut GalleryState, a: ToggleAccordion, _| {
                                    s.accordion_open = if s.accordion_open == a.0 {
                                        usize::MAX
                                    } else {
                                        a.0
                                    }
                                })
                                    as Handler<GalleryState, ToggleAccordion>,
                            )),
                        },
                    ],
                }
                .build(ctx, view),
                // Stepper
                Stepper {
                    steps: vec![
                        "Import".into(),
                        "Configure".into(),
                        "Review".into(),
                        "Deploy".into(),
                    ],
                    active_index: 1,
                }
                .build(ctx, view),
                // Timeline
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
                }
                .build(ctx, view),
                // TreeView
                TreeView {
                    items: vec![
                        TreeItem {
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
                                    on_select: Some(ctx.bind(
                                        SelectTreeNode("main".into()),
                                        (|s: &mut GalleryState, a: SelectTreeNode, _| {
                                            s.tree_selected = Some(a.0)
                                        })
                                            as Handler<GalleryState, SelectTreeNode>,
                                    )),
                                },
                                TreeItem {
                                    id: "lib".into(),
                                    label: "lib.rs".into(),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(ctx.bind(
                                        SelectTreeNode("lib".into()),
                                        (|s: &mut GalleryState, a: SelectTreeNode, _| {
                                            s.tree_selected = Some(a.0)
                                        })
                                            as Handler<GalleryState, SelectTreeNode>,
                                    )),
                                },
                            ],
                            on_toggle: Some(ctx.bind(
                                ToggleTreeNode("src".into()),
                                (|s: &mut GalleryState, a: ToggleTreeNode, _| {
                                    if !s.tree_expanded.remove(&a.0) {
                                        s.tree_expanded.insert(a.0);
                                    }
                                })
                                    as Handler<GalleryState, ToggleTreeNode>,
                            )),
                            on_select: None,
                        },
                    ],
                    expanded_ids: s.tree_expanded.clone(),
                    selected_id: s.tree_selected.clone(),
                }
                .build(ctx, view),
            ],
        );

        // -- Overlay Widgets --
        let overlay_section = section(
            "Overlays",
            vec![
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Box::new(Text::new("Open Modal").into_node())),
                            on_press: Some(ctx.bind(
                                ToggleModal,
                                (|s: &mut GalleryState, _, _| s.modal_open = !s.modal_open)
                                    as Handler<GalleryState, ToggleModal>,
                            )),
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Box::new(Text::new("Open Drawer").into_node())),
                            on_press: Some(ctx.bind(
                                ToggleDrawer,
                                (|s: &mut GalleryState, _, _| s.drawer_open = !s.drawer_open)
                                    as Handler<GalleryState, ToggleDrawer>,
                            )),
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Box::new(Text::new("Show Toast").into_node())),
                            on_press: Some(ctx.bind(
                                ShowToast,
                                (|s: &mut GalleryState, _, _| s.show_toast = true)
                                    as Handler<GalleryState, ShowToast>,
                            )),
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                }
                .into_node(),
                // Tooltip
                Tooltip {
                    id: WidgetNodeId::explicit("gallery_tooltip"),
                    child: Box::new(
                        Text::new("Hover me for tooltip").into_node(),
                    ),
                    text: "This is a tooltip!".into(),
                    is_visible: false,
                }
                .build(ctx, view),
                // Select
                Select {
                    id: WidgetNodeId::explicit("gallery_select"),
                    selected_label: s.select_value.clone(),
                    items: vec![
                        SelectItem {
                            label: "Option A".into(),
                            icon: None,
                            on_select: ctx.bind(
                                SelectValue("Option A".into()),
                                (|s: &mut GalleryState, a: SelectValue, _| {
                                    s.select_value = Some(a.0);
                                    s.select_open = false;
                                }) as Handler<GalleryState, SelectValue>,
                            ),
                        },
                        SelectItem {
                            label: "Option B".into(),
                            icon: None,
                            on_select: ctx.bind(
                                SelectValue("Option B".into()),
                                (|s: &mut GalleryState, a: SelectValue, _| {
                                    s.select_value = Some(a.0);
                                    s.select_open = false;
                                }) as Handler<GalleryState, SelectValue>,
                            ),
                        },
                    ],
                    is_open: s.select_open,
                    on_toggle: Some(ctx.bind(
                        ToggleSelect,
                        (|s: &mut GalleryState, _, _| s.select_open = !s.select_open)
                            as Handler<GalleryState, ToggleSelect>,
                    )),
                    placeholder: "Choose...".into(),
                    width: Some(200.0),
                }
                .build(ctx, view),
            ],
        );

        // -- Register Portals for Modal/Drawer/Toast --
        if s.modal_open {
            Modal {
                id: WidgetNodeId::explicit("gallery_modal"),
                title: "Gallery Modal".into(),
                content: Box::new(
                    Text::new("This is modal content.\nYou can put any widget here.")
                        .into_node(),
                ),
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    ToggleModal,
                    (|s: &mut GalleryState, _, _| s.modal_open = false)
                        as Handler<GalleryState, ToggleModal>,
                )),
                actions: vec![
                    ModalAction {
                        label: "Cancel".into(),
                        on_press: Some(ctx.bind(
                            ToggleModal,
                            (|s: &mut GalleryState, _, _| s.modal_open = false)
                                as Handler<GalleryState, ToggleModal>,
                        )),
                        is_primary: false,
                    },
                    ModalAction {
                        label: "Confirm".into(),
                        on_press: Some(ctx.bind(
                            ToggleModal,
                            (|s: &mut GalleryState, _, _| s.modal_open = false)
                                as Handler<GalleryState, ToggleModal>,
                        )),
                        is_primary: true,
                    },
                ],
                width: None,
            }
            .build(ctx, view);
        }

        if s.drawer_open {
            Drawer {
                id: WidgetNodeId::explicit("gallery_drawer"),
                side: DrawerSide::Right,
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    ToggleDrawer,
                    (|s: &mut GalleryState, _, _| s.drawer_open = false)
                        as Handler<GalleryState, ToggleDrawer>,
                )),
                content: Box::new(
                    VStack {
                        spacing: Some(12.0),
                        children: vec![
                            Text::new("Drawer Content").size(18.0).into_node(),
                            Text::new("This slides in from the right.").into_node(),
                        ],
                    }
                    .into_node(),
                ),
                width: Some(280.0),
            }
            .build(ctx, view);
        }

        if s.show_toast {
            let toast = Toast {
                id: WidgetNodeId::explicit("gallery_toast"),
                kind: ToastKind::Success,
                message: "Action completed!".into(),
                on_close: Some(ctx.bind(
                    DismissToast,
                    (|s: &mut GalleryState, _, _| s.show_toast = false)
                        as Handler<GalleryState, DismissToast>,
                )),
            }
            .build(ctx, view);
            ctx.register_portal_with_layer(
                fission_core::PortalLayer::Toast,
                Some(WidgetNodeId::explicit("gallery_toast")),
                fission_widgets::Positioned {
                    right: Some(20.0),
                    bottom: Some(20.0),
                    width: Some(320.0),
                    child: Some(Box::new(toast)),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        // -- Compose everything --
        let all_sections = VStack {
            spacing: Some(16.0),
            children: vec![
                Container::new(Text::new("Fission Widget Gallery").size(28.0).into_node())
                    .padding_all(16.0)
                    .into_node(),
                display_section,
                input_section,
                feedback_section,
                nav_section,
                data_section,
                overlay_section,
                Spacer {
                    height: Some(40.0),
                    ..Default::default()
                }
                .into_node(),
            ],
        }
        .into_node();

        Scroll {
            direction: FlexDirection::Column,
            child: Some(Box::new(
                Container::new(all_sections)
                    .padding_all(24.0)
                    .flex_grow(1.0)
                    .into_node(),
            )),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into_node()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(GalleryApp);
    app.run()
}
