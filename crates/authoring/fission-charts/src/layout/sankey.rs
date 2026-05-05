use crate::series::graph::{GraphEdge, GraphNode};
use std::collections::HashMap;

pub struct SankeyLayout;

impl SankeyLayout {
    pub fn compute(
        nodes: &[GraphNode],
        edges: &[GraphEdge],
        width: f32,
        height: f32,
    ) -> (
        HashMap<String, fission_layout::LayoutRect>,
        Vec<(String, String, String)>,
    ) {
        let mut rects = HashMap::new();
        let mut paths = Vec::new();

        let mut sources = Vec::new();
        let mut targets = Vec::new();

        for n in nodes {
            let is_source = edges.iter().any(|e| e.source == n.id);
            let is_target = edges.iter().any(|e| e.target == n.id);
            if is_source && !is_target {
                sources.push(n);
            } else {
                targets.push(n);
            }
        }

        if sources.is_empty() {
            sources = nodes.iter().collect();
            targets = vec![];
        }

        let node_w = 20.0;
        let gap_y = 10.0;

        // Layout sources
        let source_h =
            (height - gap_y * (sources.len().max(1) - 1) as f32) / sources.len().max(1) as f32;
        for (i, s) in sources.iter().enumerate() {
            let y = i as f32 * (source_h + gap_y);
            rects.insert(
                s.id.clone(),
                fission_layout::LayoutRect::new(0.0, y, node_w, source_h),
            );
        }

        // Layout targets
        let target_h =
            (height - gap_y * (targets.len().max(1) - 1) as f32) / targets.len().max(1) as f32;
        for (i, t) in targets.iter().enumerate() {
            let y = i as f32 * (target_h + gap_y);
            rects.insert(
                t.id.clone(),
                fission_layout::LayoutRect::new(width - node_w, y, node_w, target_h),
            );
        }

        // Layout edges
        for e in edges {
            if let (Some(sr), Some(tr)) = (rects.get(&e.source), rects.get(&e.target)) {
                let sx = sr.right();
                let sy = sr.y() + sr.height() / 2.0;
                let tx = tr.x();
                let ty = tr.y() + tr.height() / 2.0;

                let cp1x = sx + (tx - sx) / 2.0;
                let cp2x = tx - (tx - sx) / 2.0;

                // Flow width based on some metric, let's use fixed 10.0 for now
                let w = 10.0;
                let sy_top = sy - w / 2.0;
                let sy_bot = sy + w / 2.0;
                let ty_top = ty - w / 2.0;
                let ty_bot = ty + w / 2.0;

                let path = format!(
                    "M {} {} C {} {} {} {} {} {} L {} {} C {} {} {} {} {} {} Z",
                    sx,
                    sy_top,
                    cp1x,
                    sy_top,
                    cp2x,
                    ty_top,
                    tx,
                    ty_top,
                    tx,
                    ty_bot,
                    cp2x,
                    ty_bot,
                    cp1x,
                    sy_bot,
                    sx,
                    sy_bot
                );
                paths.push((e.source.clone(), e.target.clone(), path));
            }
        }

        (rects, paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sankey_layout() {
        let nodes = vec![
            GraphNode {
                id: "1".into(),
                name: "A".into(),
                value: 10.0,
            },
            GraphNode {
                id: "2".into(),
                name: "B".into(),
                value: 10.0,
            },
        ];
        let edges = vec![GraphEdge {
            source: "1".into(),
            target: "2".into(),
        }];

        let (rects, paths) = SankeyLayout::compute(&nodes, &edges, 100.0, 100.0);
        assert_eq!(rects.len(), 2);
        assert_eq!(paths.len(), 1);
    }
}
