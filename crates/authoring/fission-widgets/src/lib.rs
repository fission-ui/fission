pub use fission_core::ui::widgets::Icon;
pub use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Checkbox, Column, Container, CustomNode, FocusScope,
    Grid, GridItem, Image, LazyColumn, Node, Overlay, Positioned, Radio, Row, SafeArea, Scroll,
    Slider, Spacer, Switch, Text, TextContent, TextInput, Video, ZStack,
};
pub use fission_core::view::{Selector, View, Widget};
pub use fission_core::BuildCtx;

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

pub mod stepper;
pub use stepper::Stepper;

pub mod link;
pub use link::Link;

pub mod pagination;
pub use pagination::Pagination;

pub mod popover;
pub use popover::Popover;

pub mod router;
pub use router::{Route, RouteParams, Router};

use fission_core::{
    lowering::NodeBuilder, op::StructuralOp, LowerDyn, LoweringContext, NodeId, Op,
};
use std::sync::Arc;

// Canvas (CustomPaint) convenience
pub struct CanvasLowerer {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub painter: Arc<dyn Fn(&mut LoweringContext) -> Vec<NodeId> + Send + Sync>,
}

impl std::fmt::Debug for CanvasLowerer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasLowerer")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl LowerDyn for CanvasLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let child_ids = (self.painter)(cx);
        let group_id = cx.next_node_id();
        let mut group =
            NodeBuilder::new(group_id, Op::Structural(StructuralOp::Group { stable_hash: 0 }));
        for cid in child_ids {
            group.add_child(cid);
        }
        let group_node = group.build(cx);

        let mut root = NodeBuilder::new(
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

pub fn canvas<F>(width: Option<f32>, height: Option<f32>, painter: F) -> Node
where
    F: Fn(&mut LoweringContext) -> Vec<NodeId> + Send + Sync + 'static,
{
    Node::Custom(fission_core::CustomNode {
        debug_tag: "Canvas".into(),
        lowerer: Some(Arc::new(CanvasLowerer {
            width,
            height,
            painter: Arc::new(painter),
        })),
    })
}

// AbsoluteFill convenience
#[derive(Debug)]
struct AbsoluteFillLowerer {
    child: Node,
}

impl LowerDyn for AbsoluteFillLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let child_id = self.child.lower(cx);
        let mut builder = NodeBuilder::new(
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

pub fn absolute_fill(child: Node) -> Node {
    Node::Custom(fission_core::CustomNode {
        debug_tag: "AbsoluteFill".into(),
        lowerer: Some(Arc::new(AbsoluteFillLowerer { child })),
    })
}

// Flyout (anchor-relative absolute positioning) convenience
#[derive(Debug)]
struct FlyoutLowerer {
    anchor: NodeId,
    content: Node,
}

impl LowerDyn for FlyoutLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let content_id = self.content.lower(cx);
        // Create a marker node that tells the layout engine to reposition `content_id`
        let marker_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Flyout {
                anchor: self.anchor,
                content: content_id,
            }),
        )
        .build(cx);

        // Ensure both the content and marker are attached to the tree via a structural group.
        let mut wrapper = NodeBuilder::new(
            cx.next_node_id(),
            Op::Structural(StructuralOp::Group { stable_hash: 0 }),
        );
        wrapper.add_child(content_id);
        wrapper.add_child(marker_id);
        wrapper.build(cx)
    }
}

pub fn flyout(anchor: NodeId, content: Node) -> Node {
    Node::Custom(fission_core::CustomNode {
        debug_tag: "Flyout".into(),
        lowerer: Some(Arc::new(FlyoutLowerer { anchor, content })),
    })
}

// Portal
#[derive(Debug, Clone)]
pub struct Portal {
    pub child: Node,
}

impl<S: fission_core::AppState> Widget<S> for Portal {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        ctx.register_portal(self.child.clone());
        // Return invisible spacer
        Node::Spacer(fission_core::ui::widgets::spacer::Spacer::default())
    }
}
