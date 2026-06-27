use fission::core::op::Color as IrColor;
use fission::core::ui::{
    Button, ButtonVariant, Checkbox, Container, Scroll, Slider, Switch, Text, TextInput, Widget,
};
use fission::core::{reduce_with, ActionEnvelope, FlexDirection, GlobalState, WidgetId};
use fission::prelude::fission_action;
use fission::prelude::DesktopApp;
use fission::widgets::{
    Accordion, AccordionItem, Alert, AlertKind, Avatar, Badge, Breadcrumb, BreadcrumbItem, Card,
    CircularProgress, Code, Divider, Drawer, DrawerSide, EmptyState, HStack, Kbd, Link, MenuButton,
    MenuItem, Modal, ModalAction, NumberInput, Pagination, ProgressBar, SegmentedControl, Select,
    SelectItem, Skeleton, SkeletonMotion, Spacer, Spinner, SpinnerMotion, Stat, Stepper, TabItem,
    Tabs, Tag, Timeline, TimelineItem, Toast, ToastKind, Tooltip, TreeItem, TreeView, VStack,
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
            segmented_idx: 0,
            current_page: 1,
            tree_expanded,
            tree_selected: None,
            show_toast: false,
        }
    }
}

impl GlobalState for GalleryState {}

// --- Actions ---

#[fission_action(no_eq)]
#[serde(transparent)]
struct SetSlider(f32);

#[fission_action]
struct ToggleChecked;

#[fission_action]
struct ToggleSwitch;

#[fission_action]
#[serde(transparent)]
struct UpdateText(String);

#[fission_action(no_eq)]
struct IncrementNumber;

#[fission_action(no_eq)]
struct DecrementNumber;

#[fission_action]
struct SetTab(usize);

#[fission_action]
struct ToggleAccordion(usize);

#[fission_action]
struct ToggleSelect;

#[fission_action]
struct SelectValue(String);

#[fission_action]
struct ToggleMenu;

#[fission_action]
struct ToggleModal;

#[fission_action]
struct ToggleDrawer;

#[fission_action]
struct SetSegmented(usize);

#[fission_action]
struct SetPage(usize);

#[fission_action]
struct ToggleTreeNode(String);

#[fission_action]
struct SelectTreeNode(String);

#[fission_action]
struct DismissToast;

#[fission_action]
struct ShowToast;

#[fission_action]
struct Noop;

// --- Helpers ---

fn section(title: &str, children: Vec<Widget>) -> Widget {
    VStack {
        spacing: Some(8.0),
        children: vec![
            vec![
                Spacer {
                    height: Some(8.0),
                    ..Default::default()
                }
                .into(),
                Text::new(title).size(20.0).into(),
                Divider::default().build_inline(),
            ],
            children,
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
    .into()
}

trait BuildInline {
    fn build_inline(self) -> Widget;
}

impl BuildInline for Divider {
    fn build_inline(self) -> Widget {
        Container::new(fission::core::ui::widgets::Spacer::default())
            .height(1.0)
            .bg(IrColor {
                r: 200,
                g: 200,
                b: 200,
                a: 255,
            })
            .flex_grow(1.0)
            .into()
    }
}

// --- App Widget ---

#[derive(Clone)]
struct GalleryApp;

impl From<GalleryApp> for Widget {
    fn from(_component: GalleryApp) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let s = view.state();
        let tokens = &view.env().theme.tokens;
        let viewport_width = view.viewport_size().width.max(0.0);
        let control_width = (viewport_width - 96.0).clamp(220.0, 420.0);
        let drawer_width = (viewport_width * 0.42).clamp(220.0, 340.0);

        // -- Display Widgets --
        let display_section = section(
            "Display",
            vec![
                HStack {
                    spacing: Some(12.0),
                    children: vec![
                        Text::new("Hello Fission").size(16.0).into(),
                        Badge {
                            text: "New".into(),
                            ..Default::default()
                        }
                        .into(),
                        Tag {
                            label: "Rust".into(),
                            on_close: None,
                        }
                        .into(),
                        Avatar {
                            name: Some("John Doe".into()),
                            src: None,
                            size: Some(36.0),
                        }
                        .into(),
                    ],
                }
                .into(),
                HStack {
                    spacing: Some(12.0),
                    children: vec![
                        Code {
                            text: "let x = 42;".into(),
                        }
                        .into(),
                        Kbd {
                            text: "Ctrl+C".into(),
                        }
                        .into(),
                    ],
                }
                .into(),
                Stat {
                    label: "Total Users".into(),
                    value: "1,234".into(),
                    help_text: Some("+12% this month".into()),
                }
                .into(),
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
                            child: Some(Text::new("Filled").into()),
                            on_press: Some(ctx.bind(Noop, reduce_with!((|_, _: Noop, _| {})))),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Outline").into()),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Text::new("Ghost").into()),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Text::new("Disabled").into()),
                            disabled: true,
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
                // TextInput
                TextInput {
                    value: s.text_value.clone(),
                    placeholder: Some("Type something...".into()),
                    on_change: Some(ctx.bind(
                        UpdateText(String::new()),
                        reduce_with!((|s: &mut GalleryState, a: UpdateText, _| s.text_value = a.0)),
                    )),
                    width: Some(control_width),
                    ..Default::default()
                }
                .into(),
                // Checkbox + Switch + Radio
                HStack {
                    spacing: Some(16.0),
                    children: vec![
                        Checkbox {
                            checked: s.checked,
                            on_toggle: Some(ctx.bind(
                                ToggleChecked,
                                reduce_with!((|s: &mut GalleryState, _, _| s.checked = !s.checked)),
                            )),
                            label: Some("Check me".into()),
                            ..Default::default()
                        }
                        .into(),
                        Switch {
                            checked: s.switch_on,
                            on_toggle: Some(ctx.bind(
                                ToggleSwitch,
                                reduce_with!(
                                    (|s: &mut GalleryState, _, _| s.switch_on = !s.switch_on)
                                ),
                            )),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
                // Slider
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Text::new("Slider:").into(),
                        Container::new(Slider {
                            value: s.slider_val,
                            min: 0.0,
                            max: 100.0,
                            on_change: Some(ctx.bind(
                                SetSlider(0.0),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: SetSlider, _| s.slider_val = a.0)
                                ),
                            )),
                            ..Default::default()
                        })
                        .width(control_width.min(280.0))
                        .into(),
                        Text::new(format!("{:.0}", s.slider_val)).into(),
                    ],
                }
                .into(),
                // NumberInput
                NumberInput {
                    value: s.number_val,
                    step: 1.0,
                    on_increment: Some(ctx.bind(
                        IncrementNumber,
                        reduce_with!((|s: &mut GalleryState, _, _| s.number_val += 1.0)),
                    )),
                    on_decrement: Some(ctx.bind(
                        DecrementNumber,
                        reduce_with!((|s: &mut GalleryState, _, _| s.number_val -= 1.0)),
                    )),
                    ..Default::default()
                }
                .into(),
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
                .into(),
                Alert {
                    kind: AlertKind::Success,
                    title: "Success".into(),
                    description: None,
                }
                .into(),
                Alert {
                    kind: AlertKind::Warning,
                    title: "Warning".into(),
                    description: Some("Be careful!".into()),
                }
                .into(),
                Alert {
                    kind: AlertKind::Error,
                    title: "Error".into(),
                    description: Some("Something went wrong.".into()),
                }
                .into(),
                HStack {
                    spacing: Some(16.0),
                    children: vec![ProgressBar { value: 0.65 }.into()],
                }
                .into(),
                HStack {
                    spacing: Some(16.0),
                    children: vec![
                        Spinner {
                            id: WidgetId::explicit("spinner1"),
                            color: None,
                            motion: Some(SpinnerMotion::Default),
                        }
                        .into(),
                        CircularProgress {
                            value: Some(0.7),
                            size: 40.0,
                            ..Default::default()
                        }
                        .into(),
                        Skeleton {
                            id: WidgetId::explicit("skel1"),
                            width: Some(120.0),
                            height: Some(20.0),
                            circle: false,
                            motion: Some(SkeletonMotion::Default),
                        }
                        .into(),
                    ],
                }
                .into(),
                EmptyState {
                    icon: None,
                    title: "No items yet".into(),
                    description: Some("Add your first item to get started.".into()),
                    action: Some(
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Add Item").into()),
                            ..Default::default()
                        }
                        .into(),
                    ),
                }
                .into(),
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
                            content: Text::new("Content of Tab A").into(),
                            on_press: Some(ctx.bind(
                                SetTab(0),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: SetTab, _| s.active_tab = a.0)
                                ),
                            )),
                        },
                        TabItem {
                            title: "Tab B".into(),
                            content: Text::new("Content of Tab B").into(),
                            on_press: Some(ctx.bind(
                                SetTab(1),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: SetTab, _| s.active_tab = a.0)
                                ),
                            )),
                        },
                        TabItem {
                            title: "Tab C".into(),
                            content: Text::new("Content of Tab C").into(),
                            on_press: Some(ctx.bind(
                                SetTab(2),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: SetTab, _| s.active_tab = a.0)
                                ),
                            )),
                        },
                    ],
                    ..Default::default()
                }
                .into(),
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
                .into(),
                // SegmentedControl
                SegmentedControl {
                    options: vec!["Day".into(), "Week".into(), "Month".into()],
                    selected_index: s.segmented_idx,
                    on_change: Some(Arc::new({
                        let env = ctx.bind(
                            SetSegmented(0),
                            reduce_with!(
                                (|s: &mut GalleryState, a: SetSegmented, _| s.segmented_idx = a.0)
                            ),
                        );
                        move |idx| ActionEnvelope {
                            id: env.id,
                            payload: serde_json::to_vec(&idx).unwrap(),
                        }
                    })),
                }
                .into(),
                // Pagination
                Pagination {
                    current_page: s.current_page.max(1),
                    total_pages: 10,
                    on_change: Some(Arc::new({
                        let env = ctx.bind(
                            SetPage(1),
                            reduce_with!(
                                (|s: &mut GalleryState, a: SetPage, _| s.current_page = a.0)
                            ),
                        );
                        move |page| ActionEnvelope {
                            id: env.id,
                            payload: serde_json::to_vec(&page).unwrap(),
                        }
                    })),
                }
                .into(),
                // Link
                Link {
                    text: "Visit documentation".into(),
                    on_click: None,
                }
                .into(),
                // MenuButton
                MenuButton {
                    id: WidgetId::explicit("gallery_menu"),
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
                        reduce_with!((|s: &mut GalleryState, _, _| s.menu_open = !s.menu_open)),
                    )),
                }
                .into(),
            ],
        );

        // -- Data Widgets --
        let data_section = section(
            "Data Display",
            vec![
                // Card
                Card {
                    child: VStack {
                        spacing: Some(4.0),
                        children: vec![
                            Text::new("Card Title").size(18.0).into(),
                            Text::new("Some card content goes here.")
                                .color(tokens.colors.text_secondary)
                                .into(),
                        ],
                    }
                    .into(),
                    ..Default::default()
                }
                .into(),
                // Accordion
                Accordion {
                    items: vec![
                        AccordionItem {
                            title: "Section 1".into(),
                            content: Text::new("Content of section 1").into(),
                            is_expanded: s.accordion_open == 0,
                            on_toggle: Some(ctx.bind(
                                ToggleAccordion(0),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: ToggleAccordion, _| {
                                        s.accordion_open = if s.accordion_open == a.0 {
                                            usize::MAX
                                        } else {
                                            a.0
                                        }
                                    })
                                ),
                            )),
                        },
                        AccordionItem {
                            title: "Section 2".into(),
                            content: Text::new("Content of section 2").into(),
                            is_expanded: s.accordion_open == 1,
                            on_toggle: Some(ctx.bind(
                                ToggleAccordion(1),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: ToggleAccordion, _| {
                                        s.accordion_open = if s.accordion_open == a.0 {
                                            usize::MAX
                                        } else {
                                            a.0
                                        }
                                    })
                                ),
                            )),
                        },
                    ],
                    motion: None,
                }
                .into(),
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
                .into(),
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
                .into(),
                // TreeView
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
                                on_select: Some(ctx.bind(
                                    SelectTreeNode("main".into()),
                                    reduce_with!(
                                        (|s: &mut GalleryState, a: SelectTreeNode, _| {
                                            s.tree_selected = Some(a.0)
                                        })
                                    ),
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
                                    reduce_with!(
                                        (|s: &mut GalleryState, a: SelectTreeNode, _| {
                                            s.tree_selected = Some(a.0)
                                        })
                                    ),
                                )),
                            },
                        ],
                        on_toggle: Some(ctx.bind(
                            ToggleTreeNode("src".into()),
                            reduce_with!(
                                (|s: &mut GalleryState, a: ToggleTreeNode, _| {
                                    if !s.tree_expanded.remove(&a.0) {
                                        s.tree_expanded.insert(a.0);
                                    }
                                })
                            ),
                        )),
                        on_select: None,
                    }],
                    expanded_ids: s.tree_expanded.clone(),
                    selected_id: s.tree_selected.clone(),
                }
                .into(),
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
                            child: Some(Text::new("Open Modal").into()),
                            on_press: Some(ctx.bind(
                                ToggleModal,
                                reduce_with!(
                                    (|s: &mut GalleryState, _, _| s.modal_open = !s.modal_open)
                                ),
                            )),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Open Drawer").into()),
                            on_press: Some(ctx.bind(
                                ToggleDrawer,
                                reduce_with!(
                                    (|s: &mut GalleryState, _, _| s.drawer_open = !s.drawer_open)
                                ),
                            )),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Show Toast").into()),
                            on_press: Some(ctx.bind(
                                ShowToast,
                                reduce_with!((|s: &mut GalleryState, _, _| s.show_toast = true)),
                            )),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
                // Tooltip
                Tooltip {
                    id: WidgetId::explicit("gallery_tooltip"),
                    child: Text::new("Hover me for tooltip").into(),
                    text: "This is a tooltip!".into(),
                    is_visible: false,
                    motion: None,
                }
                .into(),
                // Select
                Select {
                    id: WidgetId::explicit("gallery_select"),
                    selected_label: s.select_value.clone(),
                    items: vec![
                        SelectItem {
                            label: "Option A".into(),
                            icon: None,
                            on_select: ctx.bind(
                                SelectValue("Option A".into()),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: SelectValue, _| {
                                        s.select_value = Some(a.0);
                                        s.select_open = false;
                                    })
                                ),
                            ),
                        },
                        SelectItem {
                            label: "Option B".into(),
                            icon: None,
                            on_select: ctx.bind(
                                SelectValue("Option B".into()),
                                reduce_with!(
                                    (|s: &mut GalleryState, a: SelectValue, _| {
                                        s.select_value = Some(a.0);
                                        s.select_open = false;
                                    })
                                ),
                            ),
                        },
                    ],
                    is_open: s.select_open,
                    on_toggle: Some(ctx.bind(
                        ToggleSelect,
                        reduce_with!((|s: &mut GalleryState, _, _| s.select_open = !s.select_open)),
                    )),
                    placeholder: "Choose...".into(),
                    width: Some(control_width.min(260.0)),
                }
                .into(),
            ],
        );

        // -- Register Portals for Modal/Drawer/Toast --
        if s.modal_open {
            let _: Widget = Modal {
                id: WidgetId::explicit("gallery_modal"),
                title: "Gallery Modal".into(),
                content: Text::new("This is modal content.\nYou can put any widget here.").into(),
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    ToggleModal,
                    reduce_with!((|s: &mut GalleryState, _, _| s.modal_open = false)),
                )),
                actions: vec![
                    ModalAction {
                        label: "Cancel".into(),
                        on_press: Some(ctx.bind(
                            ToggleModal,
                            reduce_with!((|s: &mut GalleryState, _, _| s.modal_open = false)),
                        )),
                        is_primary: false,
                    },
                    ModalAction {
                        label: "Confirm".into(),
                        on_press: Some(ctx.bind(
                            ToggleModal,
                            reduce_with!((|s: &mut GalleryState, _, _| s.modal_open = false)),
                        )),
                        is_primary: true,
                    },
                ],
                width: None,
                motion: None,
            }
            .into();
        }

        if s.drawer_open {
            let _: Widget = Drawer {
                id: WidgetId::explicit("gallery_drawer"),
                side: DrawerSide::Right,
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    ToggleDrawer,
                    reduce_with!((|s: &mut GalleryState, _, _| s.drawer_open = false)),
                )),
                content: VStack {
                    spacing: Some(12.0),
                    children: vec![
                        Text::new("Drawer Content").size(18.0).into(),
                        Text::new("This slides in from the right.").into(),
                    ],
                }
                .into(),
                width: Some(drawer_width),
                motion: None,
            }
            .into();
        }

        if s.show_toast {
            let toast: Widget = Toast {
                id: WidgetId::explicit("gallery_toast"),
                kind: ToastKind::Success,
                message: "Action completed!".into(),
                on_close: Some(ctx.bind(
                    DismissToast,
                    reduce_with!((|s: &mut GalleryState, _, _| s.show_toast = false)),
                )),
                motion: None,
            }
            .into();
            ctx.register_portal_with_layer(
                fission::core::PortalLayer::Toast,
                Some(WidgetId::explicit("gallery_toast")),
                fission::widgets::Positioned {
                    right: Some(20.0),
                    bottom: Some(20.0),
                    child: Some(toast),
                    ..Default::default()
                }
                .into(),
            );
        }

        // -- Compose everything --
        let all_sections: Widget = VStack {
            spacing: Some(16.0),
            children: vec![
                Container::new(Text::new("Fission Widget Gallery").size(28.0))
                    .padding_all(16.0)
                    .into(),
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
                .into(),
            ],
        }
        .into();

        Scroll {
            direction: FlexDirection::Column,
            child: Some(Container::new(all_sections).padding_all(24.0).into()),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}
fn main() -> anyhow::Result<()> {
    let app = DesktopApp::<GalleryState, _>::new(GalleryApp);
    app.run()
}
