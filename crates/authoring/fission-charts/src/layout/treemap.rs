use crate::series::treemap::TreemapNode;
use fission_layout::LayoutRect;

pub struct TreemapLayout;

impl TreemapLayout {
    pub fn squarify(
        nodes: &[TreemapNode],
        rect: LayoutRect,
    ) -> Vec<(TreemapNode, LayoutRect)> {
        let mut result = Vec::new();
        // A simplified squarify implementation that slices along the longest axis
        let total_value: f32 = nodes.iter().map(|n| n.value).sum();
        if total_value <= 0.0 { return result; }

        let mut current_x = rect.x();
        let mut current_y = rect.y();
        let mut current_w = rect.width();
        let mut current_h = rect.height();

        for node in nodes {
            let ratio = node.value / total_value;
            let area = ratio * (rect.width() * rect.height());
            
            if current_w > current_h {
                // Slice vertically
                let w = area / current_h;
                let r = fission_layout::LayoutRect::new(current_x, current_y, w, current_h);
                result.push((node.clone(), r));
                current_x += w;
                current_w -= w;
            } else {
                // Slice horizontally
                let h = area / current_w;
                let r = fission_layout::LayoutRect::new(current_x, current_y, current_w, h);
                result.push((node.clone(), r));
                current_y += h;
                current_h -= h;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treemap_squarify() {
        let nodes = vec![
            TreemapNode { name: "A".into(), value: 60.0, children: vec![] },
            TreemapNode { name: "B".into(), value: 40.0, children: vec![] },
        ];
        let rect = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
        let layout = TreemapLayout::squarify(&nodes, rect);
        
        assert_eq!(layout.len(), 2);
        assert_eq!(layout[0].1.width(), 100.0);
        assert_eq!(layout[0].1.height(), 60.0);
    }
}
