use fission::charts::{LineSegment, TreemapNode};

pub(crate) const SIMPLE_GEOJSON: &str = r#"
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "properties": { "name": "North" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[-9, 2], [-4, 3.5], [0, 2.5], [4, 4], [9, 2.5], [11, 6], [7, 10], [1, 11], [-5, 9.5], [-10, 6], [-9, 2]]]
      }
    },
    {
      "type": "Feature",
      "properties": { "name": "West" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[-9, 2], [-4, 3.5], [0, 2.5], [1, -2], [-1, -8], [-6, -9], [-11, -4], [-9, 2]]]
      }
    },
    {
      "type": "Feature",
      "properties": { "name": "East" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[0, 2.5], [4, 4], [9, 2.5], [12, -3], [8, -8], [-1, -8], [1, -2], [0, 2.5]]]
      }
    }
  ]
}
"#;

pub(crate) fn sample_tree(s: f32) -> Vec<TreemapNode> {
    vec![TreemapNode {
        name: "Fission".into(),
        value: 0.0,
        children: vec![
            TreemapNode {
                name: "Runtime".into(),
                value: 0.0,
                children: vec![
                    TreemapNode {
                        name: "Shell".into(),
                        value: 28.0 * s,
                        children: vec![],
                    },
                    TreemapNode {
                        name: "Renderer".into(),
                        value: 36.0 * s,
                        children: vec![],
                    },
                ],
            },
            TreemapNode {
                name: "Authoring".into(),
                value: 0.0,
                children: vec![
                    TreemapNode {
                        name: "Widgets".into(),
                        value: 44.0 * s,
                        children: vec![],
                    },
                    TreemapNode {
                        name: "Charts".into(),
                        value: 40.0 * s,
                        children: vec![],
                    },
                ],
            },
        ],
    }]
}

pub(crate) fn sample_lines(s: f32) -> Vec<LineSegment> {
    vec![
        LineSegment::new((-8.0, -5.0), (0.0, 7.0), 12.0 * s),
        LineSegment::new((8.0, -6.0), (0.0, 7.0), 9.0 * s),
        LineSegment::new((-7.0, 4.0), (8.0, -6.0), 6.0 * s),
        LineSegment::new((0.0, 7.0), (9.0, 2.0), 10.0 * s),
    ]
}
