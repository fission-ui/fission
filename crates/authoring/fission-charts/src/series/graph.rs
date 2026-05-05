use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSeries {
    pub name: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl GraphSeries {
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

impl Into<super::Series> for GraphSeries {
    fn into(self) -> super::Series {
        super::Series::Graph(self)
    }
}
