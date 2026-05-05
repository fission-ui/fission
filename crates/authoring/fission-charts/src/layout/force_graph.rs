use crate::series::graph::{GraphEdge, GraphNode};
use std::collections::HashMap;

pub struct ForceGraphLayout;

impl ForceGraphLayout {
    pub fn compute_positions(
        nodes: &[GraphNode],
        edges: &[GraphEdge],
        width: f32,
        height: f32,
        iterations: usize,
    ) -> HashMap<String, (f32, f32)> {
        let mut pos = HashMap::new();
        let cx = width / 2.0;
        let cy = height / 2.0;

        // Initialize in circle
        let r = width.min(height) * 0.4;
        let len = nodes.len() as f32;
        for (i, node) in nodes.iter().enumerate() {
            let angle = (i as f32 / len) * 2.0 * std::f32::consts::PI;
            pos.insert(
                node.id.clone(),
                (cx + r * angle.cos(), cy + r * angle.sin()),
            );
        }

        if len == 0.0 {
            return pos;
        }
        let k = (width * height / len).sqrt(); // optimal distance

        for _ in 0..iterations {
            let mut disp = HashMap::new();
            for n in nodes {
                disp.insert(n.id.clone(), (0.0f32, 0.0f32));
            }

            // Repulsive forces
            for i in 0..nodes.len() {
                for j in i + 1..nodes.len() {
                    let v = &nodes[i].id;
                    let u = &nodes[j].id;
                    let (vx, vy) = pos[v];
                    let (ux, uy) = pos[u];
                    let dx = vx - ux;
                    let dy = vy - uy;
                    let dist_sq = dx * dx + dy * dy + 0.01;
                    let dist = dist_sq.sqrt();

                    let f = (k * k) / dist;

                    let (dvx, dvy) = disp.get_mut(v).unwrap();
                    *dvx += (dx / dist) * f;
                    *dvy += (dy / dist) * f;

                    let (dux, duy) = disp.get_mut(u).unwrap();
                    *dux -= (dx / dist) * f;
                    *duy -= (dy / dist) * f;
                }
            }

            // Attractive forces
            for edge in edges {
                let v = &edge.source;
                let u = &edge.target;
                if !pos.contains_key(v) || !pos.contains_key(u) {
                    continue;
                }

                let (vx, vy) = pos[v];
                let (ux, uy) = pos[u];
                let dx = vx - ux;
                let dy = vy - uy;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);

                let f = (dist * dist) / k;

                let (dvx, dvy) = disp.get_mut(v).unwrap();
                *dvx -= (dx / dist) * f;
                *dvy -= (dy / dist) * f;

                let (dux, duy) = disp.get_mut(u).unwrap();
                *dux += (dx / dist) * f;
                *duy += (dy / dist) * f;
            }

            // Apply displacements
            for n in nodes {
                let (x, y) = pos.get_mut(&n.id).unwrap();
                let (dx, dy) = disp[&n.id];
                let disp_len = (dx * dx + dy * dy).sqrt().max(0.01);

                let max_disp = k * 0.5; // temperature

                *x += (dx / disp_len) * max_disp.min(disp_len);
                *y += (dy / disp_len) * max_disp.min(disp_len);

                // Bound to area
                *x = x.max(0.0).min(width);
                *y = y.max(0.0).min(height);
            }
        }

        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_force_graph() {
        let nodes = vec![
            GraphNode {
                id: "1".into(),
                name: "1".into(),
                value: 1.0,
            },
            GraphNode {
                id: "2".into(),
                name: "2".into(),
                value: 1.0,
            },
        ];
        let edges = vec![GraphEdge {
            source: "1".into(),
            target: "2".into(),
        }];

        let pos = ForceGraphLayout::compute_positions(&nodes, &edges, 100.0, 100.0, 10);
        assert_eq!(pos.len(), 2);
    }
}
