//! The gallery's pages, one per component, listed in a sidebar so each can be
//! reviewed on its own.

use crate::state::GalleryState;
use crate::{
    colour_picker_section, data_section, display_section, drag_drop, feedback_section,
    foundations_section, input_section, navigation_section, overlay_section,
};
use fission::prelude::*;
use serde::{Deserialize, Serialize};

const SIDEBAR_WIDTH: f32 = 232.0;

/// A page in the gallery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GalleryPage {
    #[default]
    Foundations,
    Accordion,
    Alert,
    Avatar,
    Badge,
    Breadcrumb,
    Button,
    Card,
    Checkbox,
    Code,
    ColourPicker,
    ContextMenu,
    DragAndDrop,
    Drawer,
    EmptyState,
    Link,
    Menu,
    Modal,
    NumberInput,
    Pagination,
    Progress,
    SegmentedControl,
    Select,
    Skeleton,
    Slider,
    Stat,
    Stepper,
    Switch,
    Tabs,
    Tag,
    TextInput,
    Timeline,
    Toast,
    Tooltip,
    TreeView,
}

/// Sidebar groups in display order. Components are alphabetical so a name is
/// quick to find.
pub(crate) const GROUPS: &[(&str, &[GalleryPage])] = &[
    ("Getting started", &[GalleryPage::Foundations]),
    (
        "Components",
        &[
            GalleryPage::Accordion,
            GalleryPage::Alert,
            GalleryPage::Avatar,
            GalleryPage::Badge,
            GalleryPage::Breadcrumb,
            GalleryPage::Button,
            GalleryPage::Card,
            GalleryPage::Checkbox,
            GalleryPage::Code,
            GalleryPage::ColourPicker,
            GalleryPage::ContextMenu,
            GalleryPage::DragAndDrop,
            GalleryPage::Drawer,
            GalleryPage::EmptyState,
            GalleryPage::Link,
            GalleryPage::Menu,
            GalleryPage::Modal,
            GalleryPage::NumberInput,
            GalleryPage::Pagination,
            GalleryPage::Progress,
            GalleryPage::SegmentedControl,
            GalleryPage::Select,
            GalleryPage::Skeleton,
            GalleryPage::Slider,
            GalleryPage::Stat,
            GalleryPage::Stepper,
            GalleryPage::Switch,
            GalleryPage::Tabs,
            GalleryPage::Tag,
            GalleryPage::TextInput,
            GalleryPage::Timeline,
            GalleryPage::Toast,
            GalleryPage::Tooltip,
            GalleryPage::TreeView,
        ],
    ),
];

impl GalleryPage {
    /// The name shown in the sidebar and as the page heading.
    pub fn title(self) -> &'static str {
        self.meta().0
    }

    /// A stable lowercase name for identifiers.
    pub fn slug(self) -> &'static str {
        self.meta().1
    }

    /// One line on what the component is for.
    pub fn description(self) -> &'static str {
        self.meta().2
    }

    fn meta(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Foundations => (
                "Foundations",
                "foundations",
                "The tokens every component draws from: spacing, hit targets, loading, state layers, colour pairs and window classes.",
            ),
            Self::Accordion => (
                "Accordion",
                "accordion",
                "Stacked sections that expand to reveal their content.",
            ),
            Self::Alert => (
                "Alert",
                "alert",
                "A short message that draws attention without interrupting.",
            ),
            Self::Avatar => (
                "Avatar",
                "avatar",
                "A person's picture, or their initials when there is no picture.",
            ),
            Self::Badge => ("Badge", "badge", "A small label for a status or a count."),
            Self::Breadcrumb => (
                "Breadcrumb",
                "breadcrumb",
                "Shows where the current page sits in the hierarchy.",
            ),
            Self::Button => (
                "Button",
                "button",
                "Starts an action. Choose a style by how important the action is.",
            ),
            Self::Card => (
                "Card",
                "card",
                "Groups related content and actions on one surface.",
            ),
            Self::Checkbox => (
                "Checkbox",
                "checkbox",
                "Turns an option on or off, on its own or in a list.",
            ),
            Self::Code => (
                "Code",
                "code",
                "Inline code and keyboard shortcuts.",
            ),
            Self::ColourPicker => (
                "Colour Picker",
                "colour_picker",
                "Choose a colour with any of the built-in picker styles.",
            ),
            Self::ContextMenu => (
                "Context Menu",
                "context_menu",
                "Commands shown on right-click, including Copy for selectable text.",
            ),
            Self::DragAndDrop => (
                "Drag and Drop",
                "drag_and_drop",
                "Drag task cards between lanes, see what a drop will do, or drop files onto the import target.",
            ),
            Self::Drawer => (
                "Drawer",
                "drawer",
                "A panel that slides in from the edge of the window.",
            ),
            Self::EmptyState => (
                "Empty State",
                "empty_state",
                "What to show when there is nothing here yet, and how to start.",
            ),
            Self::Link => ("Link", "link", "Goes to another page or site."),
            Self::Menu => ("Menu", "menu", "A list of actions behind a button."),
            Self::Modal => (
                "Modal",
                "modal",
                "A dialog that needs a response before you continue.",
            ),
            Self::NumberInput => (
                "Number Input",
                "number_input",
                "Enter a number, or step it up and down.",
            ),
            Self::Pagination => (
                "Pagination",
                "pagination",
                "Moves between pages of results.",
            ),
            Self::Progress => (
                "Progress",
                "progress",
                "Shows how far along a task is, or that it is still working.",
            ),
            Self::SegmentedControl => (
                "Segmented Control",
                "segmented_control",
                "Switches between a few related views.",
            ),
            Self::Select => ("Select", "select", "Choose one option from a list."),
            Self::Skeleton => (
                "Skeleton",
                "skeleton",
                "A placeholder shown while content loads.",
            ),
            Self::Slider => (
                "Slider",
                "slider",
                "Pick a value by dragging along a range.",
            ),
            Self::Stat => (
                "Stat",
                "stat",
                "A key number with its label and change.",
            ),
            Self::Stepper => (
                "Stepper",
                "stepper",
                "Shows progress through the steps of a task.",
            ),
            Self::Switch => (
                "Switch",
                "switch",
                "Turns a setting on or off straight away.",
            ),
            Self::Tabs => (
                "Tabs",
                "tabs",
                "Switches between panels of related content.",
            ),
            Self::Tag => (
                "Tag",
                "tag",
                "A compact label that can be selected or removed.",
            ),
            Self::TextInput => (
                "Text Input",
                "text_input",
                "Enter text, with a label, help text and errors.",
            ),
            Self::Timeline => (
                "Timeline",
                "timeline",
                "Events in the order they happened.",
            ),
            Self::Toast => (
                "Toast",
                "toast",
                "A brief confirmation that goes away on its own.",
            ),
            Self::Tooltip => (
                "Tooltip",
                "tooltip",
                "A short hint shown on hover or focus.",
            ),
            Self::TreeView => (
                "Tree View",
                "tree_view",
                "Browse nested items such as files and folders.",
            ),
        }
    }

    /// Pages that lay themselves out; the rest sit in a framed preview.
    fn framed(self) -> bool {
        !matches!(
            self,
            Self::Foundations | Self::DragAndDrop | Self::ColourPicker
        )
    }

    fn demos(self) -> Vec<Widget> {
        match self {
            Self::Foundations => widgets![foundations_section::FoundationsSection],
            Self::Accordion => data_section::accordion(),
            Self::Alert => feedback_section::alert(),
            Self::Avatar => display_section::avatar(),
            Self::Badge => display_section::badge(),
            Self::Breadcrumb => navigation_section::breadcrumb(),
            Self::Button => input_section::button(),
            Self::Card => data_section::card(),
            Self::Checkbox => input_section::checkbox(),
            Self::Code => display_section::code(),
            Self::ColourPicker => widgets![colour_picker_section::ColourPickerSection],
            Self::ContextMenu => display_section::context_menu(),
            Self::DragAndDrop => widgets![drag_drop::DragDropSection],
            Self::Drawer => overlay_section::drawer(),
            Self::EmptyState => feedback_section::empty_state(),
            Self::Link => navigation_section::link(),
            Self::Menu => navigation_section::menu(),
            Self::Modal => overlay_section::modal(),
            Self::NumberInput => input_section::number_input(),
            Self::Pagination => navigation_section::pagination(),
            Self::Progress => feedback_section::progress(),
            Self::SegmentedControl => navigation_section::segmented_control(),
            Self::Select => overlay_section::select(),
            Self::Skeleton => feedback_section::skeleton(),
            Self::Slider => input_section::slider(),
            Self::Stat => display_section::stat(),
            Self::Stepper => data_section::stepper(),
            Self::Switch => input_section::switch(),
            Self::Tabs => navigation_section::tabs(),
            Self::Tag => display_section::tag(),
            Self::TextInput => input_section::text_input(),
            Self::Timeline => data_section::timeline(),
            Self::Toast => overlay_section::toast(),
            Self::Tooltip => overlay_section::tooltip(),
            Self::TreeView => data_section::tree_view(),
        }
    }
}

#[fission_reducer(SelectPage)]
fn select_page(state: &mut GalleryState, page: GalleryPage) {
    state.page = page;
}

/// Every page, grouped, with the open one highlighted.
pub(crate) struct Sidebar;

impl From<Sidebar> for Widget {
    fn from(_sidebar: Sidebar) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let open = view.state().page;
        let tokens = view.env().theme.tokens.clone();
        let typography = &tokens.typography;

        let mut items: Vec<Widget> = Vec::new();
        for (index, (group, pages)) in GROUPS.iter().enumerate() {
            if index > 0 {
                items.push(
                    Spacer {
                        height: Some(tokens.spacing.m),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            items.push(
                Container::new(
                    Text::new(*group)
                        .size(typography.font_size_xs)
                        .weight(typography.font_weight_medium)
                        .color(tokens.colors.text_muted),
                )
                .padding_lengths(Length::symmetric(
                    Length::points(tokens.spacing.s),
                    Length::points(tokens.spacing.xs),
                ))
                .into(),
            );
            for page in pages.iter().copied() {
                let is_open = page == open;
                items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ComponentSize::Sm,
                        content_align: ButtonContentAlign::Start,
                        child: Some(Text::new(page.title()).into()),
                        on_press: Some(with_reducer!(ctx, SelectPage(page), select_page)),
                        // The open page reads as selected by fill and weight, not colour alone.
                        style: Some(ButtonStyleOverride {
                            background_fill: is_open.then(|| Fill::Solid(tokens.colors.primary_subtle)),
                            text_color: Some(if is_open {
                                tokens.colors.text_primary
                            } else {
                                tokens.colors.text_secondary
                            }),
                            font_weight: is_open.then_some(typography.font_weight_semibold),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                    .semantics_identifier(format!("gallery.nav.{}", page.slug()))
                    .into(),
                );
            }
        }

        let list: Widget = Container::new(Column {
            gap: Some(tokens.spacing.xxs),
            children: items,
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .into();
        let scroll: Widget = Scroll {
            direction: FlexDirection::Column,
            child: Some(list),
            show_scrollbar: false,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .semantic_label("Components")
        .into();

        Container::new(Column {
            children: widgets![scroll],
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .width(SIDEBAR_WIDTH)
        .flex_shrink(0.0)
        .bg(tokens.colors.surface)
        .into()
    }
}

/// The open page: its heading, what it is for, and its examples.
pub(crate) struct PageView;

impl From<PageView> for Widget {
    fn from(_view: PageView) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let page = view.state().page;
        let tokens = view.env().theme.tokens.clone();
        let typography = &tokens.typography;

        let demos: Widget = Column {
            gap: Some(tokens.spacing.l),
            children: page.demos(),
            ..Default::default()
        }
        .into();
        let body: Widget = if page.framed() {
            // A framed preview, so the examples read as one object apart from the page.
            Container::new(demos)
                .width_length(Length::percent(100.0))
                .padding_all(tokens.spacing.xl)
                .bg(tokens.colors.card())
                .border(tokens.colors.border, tokens.sizing.border_hairline)
                .border_radius(tokens.radii.large)
                .into()
        } else {
            demos
        };

        Column {
            gap: Some(tokens.spacing.l),
            children: widgets![
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: widgets![
                        Text::new(page.title())
                            .size(typography.heading2_size)
                            .line_height(typography.heading2_size * typography.line_height_heading)
                            .weight(typography.font_weight_semibold)
                            .color(tokens.colors.text_primary),
                        Text::new(page.description())
                            .size(typography.body_large_size)
                            .color(tokens.colors.text_secondary)
                            .wrap(true),
                    ],
                    ..Default::default()
                },
                body,
            ],
            ..Default::default()
        }
        .into()
    }
}
