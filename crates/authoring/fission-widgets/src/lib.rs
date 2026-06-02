//! High-level, composable UI widgets for the Fission framework.
//!
//! This crate provides a comprehensive widget library built on top of `fission-core`
//! primitives. Each widget follows a declarative, data-driven pattern: construct the
//! widget struct with its configuration, then convert it with `From<T> for
//! `Widget` to produce the closed widget tree.
//!
//! Widgets do not own state. They receive all data through struct fields and communicate
//! user interactions back to the application via [`ActionEnvelope`](fission_core::ActionEnvelope)
//! callbacks.
//!
//! # Widget categories
//!
//! - **Layout**: [`HStack`], [`VStack`], [`Center`], [`Wrap`], [`SplitView`], [`Divider`]
//! - **Overlays**: [`Modal`], [`Popover`], [`Tooltip`], [`Drawer`], [`Toast`], [`Portal`]
//! - **Menus**: [`Menu`], [`MenuButton`], [`MenuItem`], [`Select`], [`Combobox`], [`SegmentedControl`]
//! - **Navigation**: [`Tabs`], [`Accordion`]
//! - **Display**: [`Badge`], [`Tag`], [`Card`], [`Avatar`], [`EmptyState`], [`Icon`]
//! - **Loading**: [`ProgressBar`], [`Spinner`], [`Skeleton`], [`FutureBuilder`], [`RefreshIndicator`]
//! - **Transitions**: [`Hero`]
//!
//! # Example
//!
//! ```rust,ignore
//! use fission_widgets::{VStack, Badge, Card};
//!
//! let layout = VStack {
//!     spacing: Some(8.0),
//!     children: vec![
//!         Badge { text: "New".into(), ..Default::default() }.into(),
//!         Card { child: content }.into(),
//!     ],
//! }.into();
//! ```

pub use fission_core::ui::widgets::Icon;
pub use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Checkbox, Column, Container, CustomWidget,
    FocusScope, Grid, GridItem, Image, LazyColumn, Overlay, Positioned, Radio, Row, SafeArea,
    Scroll, Slider, Spacer, Switch, Text, TextContent, TextInput, Video, Widget, ZStack,
};
pub use fission_core::{BuildCtxHandle, Selector, ViewHandle};

pub mod dropdown;
pub use dropdown::DropDown;

pub mod stack;
pub use stack::{HStack, VStack};

pub mod badge;
pub use badge::Badge;

pub mod tag;
pub use tag::Tag;

pub mod avatar;
pub use avatar::Avatar;

pub mod divider;
pub use divider::Divider;

pub mod card;
pub use card::Card;

pub mod progress;
pub use progress::ProgressBar;

pub mod spinner;
pub use spinner::Spinner;

pub mod tabs;
pub use tabs::{TabItem, Tabs};

pub mod select;
pub use select::{Select, SelectItem};

pub mod accordion;
pub use accordion::{Accordion, AccordionItem};

pub mod tooltip;
pub use tooltip::Tooltip;
pub mod menu;
pub use menu::{Menu, MenuButton, MenuItem};

pub mod toast;
pub use toast::{Toast, ToastKind};

pub mod modal;
pub use modal::{Modal, ModalAction};

pub mod data_table;
pub use data_table::{DataTable, TableColumn, TableRow};

pub mod split_view;
pub use split_view::{SplitDirection, SplitView};

pub mod drawer;
pub use drawer::{Drawer, DrawerSide};

pub mod form_control;
pub use form_control::FormControl;

pub mod number_input;
pub use number_input::NumberInput;

pub mod alert;
pub use alert::{Alert, AlertKind};

pub mod skeleton;
pub use skeleton::Skeleton;

pub mod breadcrumb;
pub use breadcrumb::{Breadcrumb, BreadcrumbItem};

pub mod calendar;
pub use calendar::Calendar;

pub mod date_picker;
pub use date_picker::DatePicker;

pub mod time_picker;
pub use time_picker::TimePicker;

pub mod date_range_picker;
pub use date_range_picker::DateRangePicker;

pub mod combobox;
pub use combobox::Combobox;

pub mod segmented_control;
pub use segmented_control::SegmentedControl;

pub mod timeline;
pub use timeline::{Timeline, TimelineItem};

pub mod hero;
pub use hero::Hero;

pub mod web_view;
pub use web_view::WebView;

#[cfg(all(
    feature = "terminal",
    not(any(target_os = "ios", target_os = "android", target_arch = "wasm32"))
))]
pub mod terminal;
#[cfg(all(
    feature = "terminal",
    not(any(target_os = "ios", target_os = "android", target_arch = "wasm32"))
))]
pub use terminal::{TerminalLaunchConfig, TerminalSession, TerminalView};

pub mod draggable;
pub use draggable::{DragTarget, Draggable};

pub mod empty_state;
pub use empty_state::EmptyState;

pub mod file_upload;
pub use file_upload::FileUpload;

pub mod dropzone;
pub use dropzone::Dropzone;

pub mod tree_view;
pub use tree_view::{TreeItem, TreeView};

pub mod transition;
pub use transition::Transition;

pub mod simple_grid;
pub use simple_grid::SimpleGrid;

pub mod wrap;
pub use wrap::Wrap;

pub mod center;
pub use center::Center;

pub mod aspect_ratio;
pub use aspect_ratio::AspectRatio;

pub mod range_slider;
pub use range_slider::RangeSlider;

pub mod editable;
pub use editable::Editable;

pub mod code;
pub use code::{Code, Kbd};

pub mod stat;
pub use stat::Stat;

pub mod circular_progress;
pub use circular_progress::CircularProgress;

pub mod future_builder;
pub use future_builder::{AsyncConnectionState, AsyncSnapshot, AsyncWidgetBuilder, FutureBuilder};

pub mod refresh_indicator;
pub use refresh_indicator::{RefreshIndicator, RefreshIndicatorStatus};

pub mod stepper;
pub use stepper::Stepper;

pub mod link;
pub use link::Link;

pub mod markdown;
pub use markdown::MarkdownViewer;

pub mod pagination;
pub use pagination::Pagination;

pub mod popover;
pub use popover::Popover;

pub mod router;
pub use router::{Route, RouteParams, Router};

use fission_core::{
    internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx},
    op::StructuralOp,
    Op,
};
use fission_ir::WidgetId;
use std::sync::Arc;

/// Internal lowerer for the [`canvas()`] free function.
///
/// Wraps a painter closure that produces child node IDs within a `Group` node,
/// placed inside a fixed-size `Box` layout node.
pub struct CanvasLowerer {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub painter: Arc<dyn Fn(&mut InternalLoweringCx) -> Vec<WidgetId> + Send + Sync>,
}

impl std::fmt::Debug for CanvasLowerer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasLowerer")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl InternalLowerer for CanvasLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let child_ids = (self.painter)(cx);
        let group_id = cx.next_node_id();
        let mut group = InternalIrBuilder::new(
            group_id,
            Op::Structural(StructuralOp::Group { stable_hash: 0 }),
        );
        for cid in child_ids {
            group.add_child(cid);
        }
        let group_node = group.build(cx);

        let mut root = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        root.add_child(group_node);
        root.build(cx)
    }
}

/// Creates a custom paint node from a closure.
///
/// The `painter` closure receives an [`InternalLoweringCx`] and returns a list
/// of child node IDs. These are grouped inside a fixed-size box with the given
/// `width` and `height` (both optional).
pub fn canvas<F>(width: Option<f32>, height: Option<f32>, painter: F) -> Widget
where
    F: Fn(&mut InternalLoweringCx) -> Vec<WidgetId> + Send + Sync + 'static,
{
    fission_core::internal::custom_render_widget(fission_core::CustomWidget {
        debug_tag: "Canvas".into(),
        lowerer: Some(Arc::new(CanvasLowerer {
            width,
            height,
            painter: Arc::new(painter),
        })),
        render_object: None,
    })
}

// AbsoluteFill convenience
#[derive(Debug)]
struct AbsoluteFillLowerer {
    child: Widget,
}

impl InternalLowerer for AbsoluteFillLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let child_id = fission_core::internal::lower_widget(&self.child, cx);
        let mut builder = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::AbsoluteFill),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }
    fn stable_key(&self) -> u64 {
        0
    }
}

/// Wraps a child node in an `AbsoluteFill` layout node, causing it to stretch
/// to fill its parent's bounds.
pub fn absolute_fill(child: impl Into<Widget>) -> Widget {
    fission_core::internal::custom_render_widget(fission_core::internal::InternalRenderNode {
        debug_tag: "AbsoluteFill".into(),
        lowerer: Some(Arc::new(AbsoluteFillLowerer {
            child: child.into(),
        })),
        render_object: None,
    })
}

// Flyout (anchor-relative absolute positioning) convenience
#[derive(Debug)]
struct FlyoutLowerer {
    anchor: WidgetId,
    content: Widget,
}

impl InternalLowerer for FlyoutLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let content_id = fission_core::internal::lower_widget(&self.content, cx);
        // Create a marker node that tells the layout engine to reposition `content_id`
        let marker_id = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Flyout {
                anchor: self.anchor,
                content: content_id,
            }),
        )
        .build(cx);

        // Ensure both the content and marker are attached to the tree via a structural group.
        let mut wrapper = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Structural(StructuralOp::Group { stable_hash: 0 }),
        );
        wrapper.add_child(content_id);
        wrapper.add_child(marker_id);
        wrapper.build(cx)
    }
}

/// Positions `content` relative to an `anchor` node using the flyout layout system.
///
/// The layout engine places the content adjacent to the anchor's computed rect.
/// This is the foundation for [`Popover`], [`Tooltip`], [`Menu`], and [`Select`]
/// popups.
///
/// # Arguments
///
/// * `anchor` - The `WidgetId` of the widget that the flyout should be positioned relative to.
/// * `content` - The node tree to render in the flyout popup.
pub fn flyout(anchor: WidgetId, content: Widget) -> Widget {
    fission_core::internal::custom_render_widget(fission_core::CustomWidget {
        debug_tag: "Flyout".into(),
        lowerer: Some(Arc::new(FlyoutLowerer { anchor, content })),
        render_object: None,
    })
}

/// Renders its child into the overlay layer, outside the normal layout tree.
///
/// `Portal` registers its child as a portal node during build. In the rendered
/// output, the child appears above all non-portal content, composited into a
/// full-viewport `ZStack` overlay. The portal itself produces an invisible
/// spacer in the normal tree.
///
/// This is the low-level building block used by [`Modal`], [`Drawer`],
/// [`Popover`], and [`Tooltip`] to render above the main content.
#[derive(Debug, Clone)]
pub struct Portal {
    pub child: Widget,
}

impl From<Portal> for Widget {
    fn from(component: Portal) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let this = &component;

        ctx.register_portal(this.child.clone());
        // Return invisible spacer
        fission_core::ui::widgets::spacer::Spacer::default().into()
    }
}
