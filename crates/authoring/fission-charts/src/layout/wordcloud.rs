use fission_layout::LayoutRect;

pub struct WordcloudLayout;

impl WordcloudLayout {
    pub fn compute(
        words: &[(String, f32)],
        width: f32,
        height: f32,
    ) -> Vec<(String, f32, f32, f32)> {
        // text, size, x, y
        let mut result = Vec::new();
        let mut placed_rects: Vec<LayoutRect> = Vec::new();

        let cx = width / 2.0;
        let cy = height / 2.0;

        // Sort by weight descending
        let mut sorted = words.to_vec();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (word, weight) in sorted {
            let size = 10.0 + (weight / 100.0) * 30.0;
            // Approximate bounding box
            let w = word.len() as f32 * size * 0.6;
            let h = size;

            let mut angle = 0.0f32;
            let mut radius = 0.0f32;
            let mut placed = false;

            // Archimedean spiral
            for _ in 0..1000 {
                let x = cx + radius * angle.cos() - w / 2.0;
                let y = cy + radius * angle.sin() - h / 2.0;

                let rect = LayoutRect::new(x, y, w, h);

                let collision = placed_rects.iter().any(|r| {
                    !(r.x() + r.width() <= rect.x()
                        || r.x() >= rect.x() + rect.width()
                        || r.y() + r.height() <= rect.y()
                        || r.y() >= rect.y() + rect.height())
                });

                if !collision {
                    placed_rects.push(rect);
                    result.push((word.clone(), size, x, y));
                    placed = true;
                    break;
                }

                angle += 0.5;
                radius += 2.0;
            }

            if !placed {
                // Fallback to center if spiral fails
                result.push((word, size, cx, cy));
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wordcloud_layout() {
        let words = vec![("Hello".into(), 100.0), ("World".into(), 80.0)];
        let layout = WordcloudLayout::compute(&words, 500.0, 500.0);
        assert_eq!(layout.len(), 2);
    }
}
