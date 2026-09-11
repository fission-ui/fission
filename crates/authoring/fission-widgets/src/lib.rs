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
//! - **Display**: [`Badge`], [`Tag`], [`Card`], [`Avatar`], [`AvatarGroup`], [`EmptyState`], [`Icon`]
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
    Button, ButtonContent, ButtonContentAlign, ButtonMotion, ButtonStyleOverride, ButtonVariant,
    Checkbox, Column, Container, CustomWidget, FocusScope, Grid, GridItem, Image,
    IosAudioSessionCategory, IosAudioSessionCategoryOption, IosAudioSessionMode,
    IosVideoAudioOptions, LazyColumn, Overlay, Positioned, Radio, Row, SafeArea, Scroll,
    SelectionPlatformStyle, SelectionRegion, SelectionRegionControls, Slider, Spacer, Switch, Text,
    TextContent, TextInput, Video, VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy,
    Widget, ZStack,
};
pub use fission_core::{BuildCtxHandle, Selector, ViewHandle};
pub use fission_ir::{FlyoutAlignment, FlyoutOptions, FlyoutPlacement, FlyoutWidth};

#[cfg(feature = "interactive-canvas")]
pub use fission_core::ui::{
    InteractiveViewer, ViewportBoundary, ViewportClip, ViewportMargin, ViewportPanAxis,
    ViewportTransform, ViewportZoomPolicy,
};

mod motion_support;

/// Compact dropdown trigger; use `select` for the complete popup selection control.
pub mod dropdown;
pub use dropdown::DropDown;

/// Horizontal and vertical convenience stacks built on Fission flex layout.
pub mod stack;
pub use stack::{HStack, VStack};

/// Themed status, category, and count badges.
pub mod badge;
pub use badge::Badge;

/// Compact removable labels represented as tags.
pub mod tag;
pub use tag::Tag;

/// Circular image and initials-based identity avatars.
pub mod avatar;
pub use avatar::{Avatar, AvatarGroup, AvatarGroupItem};

/// Horizontal and vertical visual separators.
pub mod divider;
pub use divider::Divider;

/// Design-system card surfaces for grouping related content.
pub mod card;
pub use card::{Card, CardContent, CardDescription, CardFooter, CardHeader, CardLayout, CardTitle};

/// Determinate linear progress presentation.
pub mod progress;
pub use progress::ProgressBar;

/// Indeterminate loading spinner and its motion policy.
pub mod spinner;
pub use spinner::{Spinner, SpinnerMotion};

/// Controlled tab navigation and animated selection presentation.
pub mod tabs;
pub use tabs::{
    TabItem, TabList, TabPanel, TabPresentation, TabTrigger, TabTriggerContent, Tabs, TabsLayout,
    TabsMotion,
};

/// Controlled popup selection field and option model.
pub mod select;
pub use select::{
    Select, SelectContent, SelectEntry, SelectGroup, SelectItem, SelectLabel, SelectLayout,
    SelectOption, SelectSeparator, SelectTrigger,
};

/// Expandable sections with controlled open state.
pub mod accordion;
pub use accordion::{Accordion, AccordionItem, AccordionMotion};

/// Anchored explanatory overlay with configurable motion.
pub mod tooltip;
pub use tooltip::{Tooltip, TooltipMotion};
/// Action menus, menu items, and trigger composition.
pub mod menu;
pub use menu::{
    Menu, MenuActionItem, MenuButton, MenuButtonLayout, MenuContent, MenuEntry, MenuGroup,
    MenuItem, MenuItemTone, MenuItemTrailing, MenuLabel, MenuSeparator, MenuTrigger,
};

/// Transient status notifications and their semantic tone.
pub mod toast;
pub use toast::{Toast, ToastKind, ToastMotion};

/// Modal dialog surface, actions, and entrance/exit motion.
pub mod modal;
pub use modal::{
    Modal, ModalAction, ModalContent, ModalDescription, ModalFooter, ModalFooterAction,
    ModalHeader, ModalLayout, ModalMotion, ModalTitle,
};

/// Declarative tabular data columns, rows, and presentation.
pub mod data_table;
pub use data_table::{DataTable, TableColumn, TableRow};

/// Two-pane horizontal or vertical split layout.
pub mod split_view;
pub use split_view::{SplitDirection, SplitView};

/// Edge-mounted overlay drawer with controlled visibility.
pub mod drawer;
pub use drawer::{Drawer, DrawerMotion, DrawerSide};

/// Label, help, validation, and field composition for form controls.
pub mod form_control;
pub use form_control::{FormControl, FormControlLayout, FormDescription, FormError, FormLabel};

/// Controlled numeric stepper with increment and decrement actions.
pub mod number_input;
pub use number_input::NumberInput;

/// Persistent inline status and warning messages.
pub mod alert;
pub use alert::{
    Alert, AlertContent, AlertDescription, AlertKind, AlertLayout, AlertLeading, AlertTitle,
};

/// Placeholder loading surfaces and shimmer motion.
pub mod skeleton;
pub use skeleton::{Skeleton, SkeletonMotion};

/// Ordered navigation trail and actionable breadcrumb items.
pub mod breadcrumb;
pub use breadcrumb::{Breadcrumb, BreadcrumbEntry, BreadcrumbItem, BreadcrumbLayout};

/// Controlled month calendar and day selection.
pub mod calendar;
pub use calendar::Calendar;

/// Controlled date selection field composed with a calendar.
pub mod date_picker;
pub use date_picker::DatePicker;

/// Controlled 24-hour hour-and-minute selector.
pub mod time_picker;
pub use time_picker::TimePicker;

/// Controlled inclusive start/end date selection.
pub mod date_range_picker;
pub use date_range_picker::DateRangePicker;

/// Colour selection controls and HSVA colour representation.
pub mod colour_picker;
pub use colour_picker::{
    ColorPicker, ColorPickerVariant, ColourHsva, ColourPicker, ColourPickerVariant,
};

/// Filterable text-and-option selection control.
pub mod combobox;
pub use combobox::{
    Combobox, ComboboxContent, ComboboxEntry, ComboboxGroup, ComboboxInput, ComboboxLabel,
    ComboboxLayout, ComboboxOption, ComboboxSeparator,
};

/// Mutually exclusive selection presented as adjacent segments.
pub mod segmented_control;
pub use segmented_control::SegmentedControl;

/// Ordered event presentation with markers and supporting content.
pub mod timeline;
pub use timeline::{Timeline, TimelineEntry, TimelineItem, TimelineLayout};

/// Shared-element identity annotation for route transitions.
pub mod hero;
pub use hero::Hero;

/// Platform web-document embedding for graphical targets.
pub mod web_view;
pub use web_view::WebView;

#[cfg(all(
    feature = "terminal",
    not(any(target_os = "ios", target_os = "android", target_arch = "wasm32"))
))]
/// Native pseudo-terminal session and retained terminal-view widgets.
pub mod terminal;
#[cfg(all(
    feature = "terminal",
    not(any(target_os = "ios", target_os = "android", target_arch = "wasm32"))
))]
pub use terminal::{TerminalLaunchConfig, TerminalSession, TerminalView};

/// Internal drag sources, previews, payloads, and target declarations.
pub mod draggable;
pub use draggable::{DragPreviewOptions, DragTarget, Draggable};

/// Empty-collection messaging with optional illustration and action.
pub mod empty_state;
pub use empty_state::EmptyState;

/// File-picker trigger and selected-file presentation.
pub mod file_upload;
pub use file_upload::FileUpload;

/// Internal and external file-drop target presentation.
pub mod dropzone;
pub use dropzone::Dropzone;

/// Hierarchical controlled tree items and expansion.
pub mod tree_view;
pub use tree_view::{TreeItem, TreeView};

/// Responsive minimum-width wrapping grid.
pub mod simple_grid;
pub use simple_grid::SimpleGrid;

/// Row- or column-oriented wrapping flow layout.
pub mod wrap;
pub use wrap::Wrap;

/// Convenience centering layout.
pub mod center;
pub use center::Center;

/// Width-to-height constraint wrapper.
pub mod aspect_ratio;
pub use aspect_ratio::AspectRatio;

/// Controlled two-thumb numeric range selection.
pub mod range_slider;
pub use range_slider::RangeSlider;

/// Inline read/edit presentation for a controlled string.
pub mod editable;
pub use editable::Editable;

/// Inline code and keyboard-key text presentation.
pub mod code;
pub use code::{Code, Kbd};

/// Compact dashboard metric presentation.
pub mod stat;
pub use stat::Stat;

/// Determinate or indeterminate circular progress indicator.
pub mod circular_progress;
pub use circular_progress::{CircularProgress, CircularProgressMotion};

/// Declarative rendering of asynchronous job snapshots.
pub mod future_builder;
pub use future_builder::{AsyncConnectionState, AsyncSnapshot, AsyncWidgetBuilder, FutureBuilder};

/// Pull-to-refresh state, appearance, and action dispatch.
pub mod refresh_indicator;
pub use refresh_indicator::{RefreshIndicator, RefreshIndicatorStatus};

/// Ordered multi-step progress presentation.
pub mod stepper;
pub use stepper::Stepper;

/// Navigational link with host-lowered hyperlink metadata.
pub mod link;
pub use link::Link;

/// Markdown text rendering and scrollable document presentation.
pub mod markdown;
pub use markdown::{MarkdownContent, MarkdownViewer};

/// Controlled page-number navigation.
pub mod pagination;
pub use pagination::Pagination;

/// Anchored popup content and visibility motion.
pub mod popover;
pub use popover::{Popover, PopoverMotion};

/// Ordered declarative path routing and route-parameter capture.
pub mod router;
pub use router::{Route, RouteParams, Router};

/// Lazy access boundary for allowed, pending, denied, and redirect branches.
pub mod protected_route;
pub use protected_route::{DefaultRouteDenied, DefaultRoutePending, ProtectedRoute};

/// Geometry for revealing an anchored element through an inverse overlay.
pub mod spotlight;
pub use spotlight::Spotlight;

#[cfg(feature = "interactive-canvas")]
/// Pan/zoom canvas authoring, nodes, edges, selection, and interaction models.
pub mod infinite_canvas;
#[cfg(feature = "interactive-canvas")]
pub use infinite_canvas::{
    CanvasConnectionPreview, CanvasEdgeEndpoint, CanvasEdgeId, CanvasEdgeLabel, CanvasEdgeMarker,
    CanvasEdgeRoute, CanvasGrid, CanvasGridPattern, CanvasNodeAnchor, CanvasNodeId,
    CanvasOverlayHitTest, CanvasOverlayLayer, CanvasPortId, CanvasPortRef, CanvasSelectionPolicy,
    CanvasSnap, InfiniteCanvas, InfiniteCanvasActions, InfiniteCanvasEdge, InfiniteCanvasNode,
    InfiniteCanvasPort,
};

use fission_core::{
    internal::{IrBuilder, LowerWidget, LoweringCx},
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
    /// Optional logical width of the canvas layout box.
    pub width: Option<f32>,
    /// Optional logical height of the canvas layout box.
    pub height: Option<f32>,
    /// Painter invoked during lowering to append renderer-independent child
    /// nodes and return their stable IDs.
    pub painter: Arc<dyn Fn(&mut LoweringCx) -> Vec<WidgetId> + Send + Sync>,
}

impl std::fmt::Debug for CanvasLowerer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasLowerer")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl LowerWidget for CanvasLowerer {
    fn lower_dyn(&self, cx: &mut LoweringCx) -> WidgetId {
        let child_ids = (self.painter)(cx);
        let group_id = cx.next_node_id();
        let mut group = IrBuilder::new(
            group_id,
            Op::Structural(StructuralOp::Group { stable_hash: 0 }),
        );
        for cid in child_ids {
            group.add_child(cid);
        }
        let group_node = group.build(cx);

        let mut root = IrBuilder::new(
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
/// The `painter` closure receives an [`LoweringCx`] and returns a list
/// of child node IDs. These are grouped inside a fixed-size box with the given
/// `width` and `height` (both optional).
pub fn canvas<F>(width: Option<f32>, height: Option<f32>, painter: F) -> Widget
where
    F: Fn(&mut LoweringCx) -> Vec<WidgetId> + Send + Sync + 'static,
{
    fission_core::authoring::custom_widget(
        "Canvas",
        CanvasLowerer {
            width,
            height,
            painter: Arc::new(painter),
        },
    )
}

// AbsoluteFill convenience
#[derive(Debug)]
struct AbsoluteFillLowerer {
    child: Widget,
}

impl LowerWidget for AbsoluteFillLowerer {
    fn lower_dyn(&self, cx: &mut LoweringCx) -> WidgetId {
        let child_id = fission_core::internal::lower_widget(&self.child, cx);
        let mut builder = IrBuilder::new(
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
    fission_core::authoring::custom_widget(
        "AbsoluteFill",
        AbsoluteFillLowerer {
            child: child.into(),
        },
    )
}

// Flyout (anchor-relative absolute positioning) convenience
#[derive(Debug)]
struct FlyoutLowerer {
    anchor: WidgetId,
    content: Widget,
    options: FlyoutOptions,
}

impl LowerWidget for FlyoutLowerer {
    fn lower_dyn(&self, cx: &mut LoweringCx) -> WidgetId {
        let content_id = fission_core::internal::lower_widget(&self.content, cx);
        let mut flyout = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Flyout {
                anchor: self.anchor,
                content: content_id,
                options: self.options,
            }),
        );
        // The flyout must own its content so layout measures that content with
        // the flyout's loose constraints instead of the portal wrapper's tight
        // viewport constraints.
        flyout.add_child(content_id);
        flyout.build(cx)
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
    flyout_with_options(anchor, content, FlyoutOptions::default())
}

/// Positions `content` relative to `anchor` using an explicit retained policy.
///
/// Use this lower-level helper when a composite control needs end alignment,
/// anchor-relative sizing, a side preference, or a visible gap. Ordinary
/// popovers should use [`Popover`], which preserves the default start/auto/
/// intrinsic-width behavior.
pub fn flyout_with_options(anchor: WidgetId, content: Widget, options: FlyoutOptions) -> Widget {
    fission_core::authoring::custom_widget(
        "Flyout",
        FlyoutLowerer {
            anchor,
            content,
            options,
        },
    )
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
    /// Content registered in the window-level overlay instead of normal layout.
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
