use crate::series::map::MapSeries;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct MapRegionPath {
    pub name: String,
    pub path: String,
    pub value: Option<f32>,
}

pub struct MapLayout;

impl MapLayout {
    pub fn compute_geojson(series: &MapSeries, width: f32, height: f32) -> Vec<MapRegionPath> {
        let Some(geojson) = series.geojson.as_ref() else {
            return Vec::new();
        };
        let Ok(root) = serde_json::from_str::<Value>(geojson) else {
            return Vec::new();
        };
        let mut features = collect_features(&root, &series.name_property);
        if features.is_empty() {
            return Vec::new();
        }

        let bounds = bounds(&features);
        let value_by_name: HashMap<&str, f32> = series
            .data
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();

        features
            .drain(..)
            .filter_map(|feature| {
                let path = feature.path(width, height, bounds)?;
                Some(MapRegionPath {
                    value: value_by_name.get(feature.name.as_str()).copied(),
                    name: feature.name,
                    path,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct MapFeature {
    name: String,
    rings: Vec<Vec<(f32, f32)>>,
}

impl MapFeature {
    fn path(&self, width: f32, height: f32, bounds: GeoBounds) -> Option<String> {
        if self.rings.is_empty() {
            return None;
        }
        let mut path = String::new();
        for ring in &self.rings {
            if ring.len() < 3 {
                continue;
            }
            for (idx, (lon, lat)) in ring.iter().enumerate() {
                let (x, y) = project(*lon, *lat, width, height, bounds);
                if idx == 0 {
                    path.push_str(&format!("M {} {}", x, y));
                } else {
                    path.push_str(&format!(" L {} {}", x, y));
                }
            }
            path.push_str(" Z");
        }
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GeoBounds {
    min_lon: f32,
    max_lon: f32,
    min_lat: f32,
    max_lat: f32,
}

fn collect_features(root: &Value, name_property: &str) -> Vec<MapFeature> {
    match root.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => root
            .get("features")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|feature| feature_from_geojson_feature(feature, name_property))
            .collect(),
        Some("Feature") => feature_from_geojson_feature(root, name_property)
            .into_iter()
            .collect(),
        Some("Polygon") | Some("MultiPolygon") => geometry_rings(root)
            .map(|rings| MapFeature {
                name: "geometry".into(),
                rings,
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn feature_from_geojson_feature(feature: &Value, name_property: &str) -> Option<MapFeature> {
    let geometry = feature.get("geometry")?;
    let rings = geometry_rings(geometry)?;
    let name = feature
        .get("properties")
        .and_then(|properties| properties.get(name_property))
        .and_then(Value::as_str)
        .or_else(|| feature.get("id").and_then(Value::as_str))
        .unwrap_or("region")
        .to_string();
    Some(MapFeature { name, rings })
}

fn geometry_rings(geometry: &Value) -> Option<Vec<Vec<(f32, f32)>>> {
    match geometry.get("type").and_then(Value::as_str)? {
        "Polygon" => polygon_rings(geometry.get("coordinates")?),
        "MultiPolygon" => {
            let mut rings = Vec::new();
            for polygon in geometry.get("coordinates")?.as_array()? {
                rings.extend(polygon_rings(polygon)?);
            }
            Some(rings)
        }
        _ => None,
    }
}

fn polygon_rings(value: &Value) -> Option<Vec<Vec<(f32, f32)>>> {
    let mut rings = Vec::new();
    for ring in value.as_array()? {
        let mut points = Vec::new();
        for point in ring.as_array()? {
            let coords = point.as_array()?;
            let lon = coords.first()?.as_f64()? as f32;
            let lat = coords.get(1)?.as_f64()? as f32;
            points.push((lon, lat));
        }
        if points.len() >= 3 {
            rings.push(points);
        }
    }
    Some(rings)
}

fn bounds(features: &[MapFeature]) -> GeoBounds {
    let mut min_lon = f32::MAX;
    let mut max_lon = f32::MIN;
    let mut min_lat = f32::MAX;
    let mut max_lat = f32::MIN;
    for (lon, lat) in features
        .iter()
        .flat_map(|feature| feature.rings.iter())
        .flat_map(|ring| ring.iter())
    {
        min_lon = min_lon.min(*lon);
        max_lon = max_lon.max(*lon);
        min_lat = min_lat.min(*lat);
        max_lat = max_lat.max(*lat);
    }
    if (max_lon - min_lon).abs() < f32::EPSILON {
        max_lon += 1.0;
        min_lon -= 1.0;
    }
    if (max_lat - min_lat).abs() < f32::EPSILON {
        max_lat += 1.0;
        min_lat -= 1.0;
    }
    GeoBounds {
        min_lon,
        max_lon,
        min_lat,
        max_lat,
    }
}

fn project(lon: f32, lat: f32, width: f32, height: f32, bounds: GeoBounds) -> (f32, f32) {
    let pad = width.min(height) * 0.04;
    let usable_w = (width - pad * 2.0).max(1.0);
    let usable_h = (height - pad * 2.0).max(1.0);
    let lon_span = bounds.max_lon - bounds.min_lon;
    let lat_span = bounds.max_lat - bounds.min_lat;
    // One scale for both axes keeps region shapes; stretching each axis to the
    // plot turns maps into blocks. The spare space centres the map.
    let scale = (usable_w / lon_span).min(usable_h / lat_span);
    let offset_x = pad + (usable_w - lon_span * scale) / 2.0;
    let offset_y = pad + (usable_h - lat_span * scale) / 2.0;
    let x = offset_x + (lon - bounds.min_lon) * scale;
    let y = offset_y + (bounds.max_lat - lat) * scale;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_GEOJSON: &str = r#"
    {
      "type": "FeatureCollection",
      "features": [
        {
          "type": "Feature",
          "properties": { "name": "North" },
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]]
          }
        },
        {
          "type": "Feature",
          "properties": { "name": "South" },
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0, -10], [10, -10], [10, 0], [0, 0], [0, -10]]]
          }
        }
      ]
    }
    "#;

    #[test]
    fn map_layout_does_not_emit_regions_without_geojson() {
        let map = MapSeries::new("World", "world");
        let paths = MapLayout::compute_geojson(&map, 800.0, 600.0);
        assert!(paths.is_empty());
    }

    #[test]
    fn map_layout_projects_geojson_regions() {
        let map = MapSeries::new("World", "world")
            .geojson(SIMPLE_GEOJSON)
            .data(vec![("North", 20.0), ("South", 10.0)]);
        let paths = MapLayout::compute_geojson(&map, 800.0, 600.0);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].name, "North");
        assert_eq!(paths[0].value, Some(20.0));
        assert!(paths[0].path.starts_with('M'));
        assert!(paths[0].path.ends_with('Z'));
    }

    fn path_extent(path: &str) -> (f32, f32, f32, f32) {
        let numbers: Vec<f32> = path
            .split_whitespace()
            .filter_map(|token| token.parse::<f32>().ok())
            .collect();
        let xs = numbers.iter().step_by(2);
        let ys = numbers.iter().skip(1).step_by(2);
        let min_x = xs.clone().copied().fold(f32::MAX, f32::min);
        let max_x = xs.copied().fold(f32::MIN, f32::max);
        let min_y = ys.clone().copied().fold(f32::MAX, f32::min);
        let max_y = ys.copied().fold(f32::MIN, f32::max);
        (min_x, min_y, max_x, max_y)
    }

    #[test]
    fn map_layout_keeps_region_proportions_in_a_wide_plot() {
        // A 10x10 degree square must stay square when the plot is much wider
        // than it is tall; stretching it to fill turns maps into flat blocks.
        let map = MapSeries::new("World", "world").geojson(SIMPLE_GEOJSON);
        let paths = MapLayout::compute_geojson(&map, 800.0, 300.0);
        let (min_x, min_y, max_x, max_y) = path_extent(&paths[0].path);
        let (width, height) = (max_x - min_x, max_y - min_y);
        assert!(
            (width - height).abs() < 0.5,
            "square region projected to {width}x{height}"
        );

        // The whole map is centred horizontally in the spare width.
        let all: Vec<_> = paths.iter().map(|p| path_extent(&p.path)).collect();
        let left = all.iter().map(|e| e.0).fold(f32::MAX, f32::min);
        let right = all.iter().map(|e| e.2).fold(f32::MIN, f32::max);
        assert!((left - (800.0 - right)).abs() < 0.5, "map not centred: {left}..{right}");
    }
}
