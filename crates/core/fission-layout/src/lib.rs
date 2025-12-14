use fission_ir::{NodeId, FlexDirection as IrFlexDirection}; 
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use taffy::prelude::*;
// use taffy::NodeId as TaffyNodeId; // Taffy 0.3 has Node as NodeId? No, Node.
use taffy::node::Node as TaffyNodeId;

pub use fission_ir::{LayoutOp, FlexDirection};

pub type LayoutUnit = f32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutPoint {
    pub x: LayoutUnit,
    pub y: LayoutUnit,
}

impl LayoutPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub fn new(x: LayoutUnit, y: LayoutUnit) -> Self { Self { x, y } }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSize {
    pub width: LayoutUnit,
    pub height: LayoutUnit,
}

impl LayoutSize {
    pub const ZERO: Self = Self { width: 0.0, height: 0.0 };
    pub fn new(width: LayoutUnit, height: LayoutUnit) -> Self { Self { width, height } }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutRect {
    pub origin: LayoutPoint,
    pub size: LayoutSize,
}

impl LayoutRect {
    pub fn new(x: LayoutUnit, y: LayoutUnit, width: LayoutUnit, height: LayoutUnit) -> Self {
        Self {
            origin: LayoutPoint { x, y },
            size: LayoutSize { width, height },
        }
    }

    pub fn x(&self) -> LayoutUnit { self.origin.x }
    pub fn y(&self) -> LayoutUnit { self.origin.y }
    pub fn width(&self) -> LayoutUnit { self.size.width }
    pub fn height(&self) -> LayoutUnit { self.size.height }

    pub fn right(&self) -> LayoutUnit { self.origin.x + self.size.width }
    pub fn bottom(&self) -> LayoutUnit { self.origin.y + self.size.height }

    pub fn contains(&self, p: LayoutPoint) -> bool {
        p.x >= self.x() && p.x < self.right() &&
        p.y >= self.y() && p.y < self.bottom()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutNodeGeometry {
    pub rect: LayoutRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSnapshot {
    pub nodes: HashMap<NodeId, LayoutNodeGeometry>,
    pub viewport_size: LayoutSize,
}

impl LayoutSnapshot {
    pub fn new(viewport_size: LayoutSize) -> Self {
        Self { nodes: HashMap::new(), viewport_size }
    }

    pub fn get_node_geometry(&self, node_id: NodeId) -> Option<&LayoutNodeGeometry> {
        self.nodes.get(&node_id)
    }

    pub fn get_node_rect(&self, node_id: NodeId) -> Option<LayoutRect> {
        self.nodes.get(&node_id).map(|g| g.rect)
    }
}

#[derive(Debug, Clone)]
pub struct LayoutInputNode {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub op: LayoutOp,
    pub children_ids: Vec<NodeId>,
    pub debug_name: String,
    pub width: Option<LayoutUnit>,
    pub height: Option<LayoutUnit>,
    pub flex_grow: LayoutUnit,
    pub flex_shrink: LayoutUnit,
    pub text_content: Option<String>,
    pub font_size: Option<f32>,
}

pub trait TextMeasurer: Send + Sync {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32);
}

pub struct LayoutEngine {
    measurer: Option<Arc<dyn TextMeasurer>>,
}

impl LayoutEngine {
    pub fn new() -> Self { Self { measurer: None } }

    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
    }

    pub fn compute_layout(
        &self,
        input_nodes: &[LayoutInputNode],
        root_node_id: NodeId,
        viewport_size: LayoutSize,
    ) -> Result<LayoutSnapshot> {
        let mut taffy = Taffy::new();
        let mut taffy_node_map: HashMap<NodeId, TaffyNodeId> = HashMap::new();
        
        let node_map: HashMap<NodeId, &LayoutInputNode> = input_nodes.iter().map(|n| (n.id, n)).collect();
        if node_map.is_empty() {
            return Err(anyhow::anyhow!("No layout nodes provided"));
        }

        if !node_map.contains_key(&root_node_id) {
            return Err(anyhow::anyhow!(
                "Root node {:?} missing from layout input set",
                root_node_id
            ));
        }

        self.build_taffy_tree(root_node_id, &mut taffy, &mut taffy_node_map, &node_map)?;

        let root_taffy_id = *taffy_node_map.get(&root_node_id).unwrap();
        taffy.compute_layout(
            root_taffy_id,
            taffy::geometry::Size {
                width: taffy::style::AvailableSpace::Definite(viewport_size.width),
                height: taffy::style::AvailableSpace::Definite(viewport_size.height),
            },
        ).map_err(|e| anyhow::anyhow!("Taffy layout error: {:?}", e))?;

        let mut geometries = HashMap::new();
        self.extract_geometry_recursive(root_node_id, LayoutPoint::ZERO, &taffy, &taffy_node_map, &node_map, &mut geometries);
        
        let mut snapshot = LayoutSnapshot::new(viewport_size);
        snapshot.nodes = geometries;
        Ok(snapshot)
    }

    fn build_taffy_tree(
        &self,
        node_id: NodeId,
        taffy: &mut Taffy,
        taffy_map: &mut HashMap<NodeId, TaffyNodeId>,
        node_map: &HashMap<NodeId, &LayoutInputNode>,
    ) -> Result<TaffyNodeId> {
        let node = node_map.get(&node_id).unwrap();
        
        let mut style = Style::default();
        
        style.flex_grow = node.flex_grow;
        style.flex_shrink = node.flex_shrink;

        match &node.op {
            LayoutOp::Box { width, height, padding } => {
                style.display = Display::Flex;
                style.align_items = Some(AlignItems::Center);
                style.justify_content = Some(JustifyContent::Center);
                
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
                };

                style.size = taffy::geometry::Size {
                    width: width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            },
            LayoutOp::Flex { direction, padding, .. } => {
                style.display = Display::Flex;
                style.flex_direction = match direction {
                    IrFlexDirection::Row => taffy::style::FlexDirection::Row,
                    IrFlexDirection::Column => taffy::style::FlexDirection::Column,
                };
                style.align_items = Some(AlignItems::Center); 
                
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
                };

                style.size = taffy::geometry::Size {
                    width: node.width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: node.height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            },
            LayoutOp::Scroll { direction } => {
                style.display = Display::Flex;
                style.flex_direction = match direction {
                    IrFlexDirection::Row => taffy::style::FlexDirection::Row,
                    IrFlexDirection::Column => taffy::style::FlexDirection::Column,
                };
                // Removed overflow property for Taffy 0.3 compatibility
                
                style.size = taffy::geometry::Size {
                    width: node.width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: node.height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            },
            LayoutOp::AbsoluteFill => {
                style.display = Display::Flex;
                style.position = Position::Absolute;
                style.inset = taffy::geometry::Rect {
                    left: points(0.0), right: points(0.0),
                    top: points(0.0), bottom: points(0.0),
                };
                style.size = taffy::geometry::Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                };
            },
            _ => {
                style.display = Display::Flex;
            }
        }

        if let (Some(text), Some(size), Some(measurer)) = (&node.text_content, node.font_size, &self.measurer) {
            let text = text.clone();
            let font_size = size;
            let measurer = measurer.clone();
            
            let child_taffy_id = taffy.new_leaf_with_measure(style, taffy::node::MeasureFunc::Boxed(Box::new(move |_known_dims, available_space| {
                let avail_width = match available_space.width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MaxContent => None,
                    AvailableSpace::MinContent => Some(0.0),
                };
                
                let (w, h) = measurer.measure(&text, font_size, avail_width);
                taffy::geometry::Size { width: w, height: h }
            }))).map_err(|e| anyhow::anyhow!("Failed to create taffy leaf: {:?}", e))?;
            
            taffy_map.insert(node_id, child_taffy_id);
            return Ok(child_taffy_id);
        }

        let mut child_taffy_ids = Vec::new();
        for child_id in &node.children_ids {
            let child_taffy_id = self.build_taffy_tree(*child_id, taffy, taffy_map, node_map)?;
            child_taffy_ids.push(child_taffy_id);
        }

        let taffy_id = taffy.new_with_children(style, &child_taffy_ids)
            .map_err(|e| anyhow::anyhow!("Failed to create taffy node: {:?}", e))?;
            
        taffy_map.insert(node_id, taffy_id);
        Ok(taffy_id)
    }

    fn extract_geometry_recursive(
        &self,
        node_id: NodeId,
        parent_absolute_pos: LayoutPoint,
        taffy: &Taffy,
        taffy_map: &HashMap<NodeId, TaffyNodeId>,
        node_map: &HashMap<NodeId, &LayoutInputNode>,
        geometries: &mut HashMap<NodeId, LayoutNodeGeometry>,
    ) {
        let taffy_id = taffy_map.get(&node_id).unwrap();
        let layout = taffy.layout(*taffy_id).unwrap();

        let absolute_x = parent_absolute_pos.x + layout.location.x;
        let absolute_y = parent_absolute_pos.y + layout.location.y;
        
        let rect = LayoutRect::new(absolute_x, absolute_y, layout.size.width, layout.size.height);
        
        geometries.insert(node_id, LayoutNodeGeometry { rect });

        let node = node_map.get(&node_id).unwrap();
        for child_id in &node.children_ids {
            self.extract_geometry_recursive(
                *child_id, 
                LayoutPoint::new(absolute_x, absolute_y), 
                taffy, 
                taffy_map, 
                node_map, 
                geometries
            );
        }
    }
}