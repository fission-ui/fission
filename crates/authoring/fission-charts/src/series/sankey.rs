use serde::{Deserialize, Serialize};
use super::graph::{GraphNode, GraphEdge};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeySeries {
    pub name: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl SankeySeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
    
    pub fn nodes(mut self, nodes: Vec<GraphNode>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn edges(mut self, edges: Vec<GraphEdge>) -> Self {
        self.edges = edges;
        self
    }
}

impl Into<super::Series> for SankeySeries {
    fn into(self) -> super::Series {
        super::Series::Sankey(self)
    }
}
