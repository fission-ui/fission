use std::f32::consts::PI;

pub fn arc(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32) -> String {
    let start_x = cx + r * start_angle.cos();
    let start_y = cy + r * start_angle.sin();
    let end_x = cx + r * end_angle.cos();
    let end_y = cy + r * end_angle.sin();
    let diff = (end_angle - start_angle).abs();
    let large_arc = if diff > PI { 1 } else { 0 };
    let sweep = if end_angle > start_angle { 1 } else { 0 };
    format!("M {} {} A {} {} 0 {} {} {} {}", start_x, start_y, r, r, large_arc, sweep, end_x, end_y)
}

pub fn pie_slice(cx: f32, cy: f32, inner_r: f32, outer_r: f32, start_angle: f32, end_angle: f32) -> String {
    if inner_r > 0.0 {
        let out_start_x = cx + outer_r * start_angle.cos();
        let out_start_y = cy + outer_r * start_angle.sin();
        let out_end_x = cx + outer_r * end_angle.cos();
        let out_end_y = cy + outer_r * end_angle.sin();
        
        let in_start_x = cx + inner_r * start_angle.cos();
        let in_start_y = cy + inner_r * start_angle.sin();
        let in_end_x = cx + inner_r * end_angle.cos();
        let in_end_y = cy + inner_r * end_angle.sin();
        
        let diff = (end_angle - start_angle).abs();
        let large_arc = if diff > PI { 1 } else { 0 };
        let sweep = if end_angle > start_angle { 1 } else { 0 };
        
        format!("M {} {} A {} {} 0 {} {} {} {} L {} {} A {} {} 0 {} {} {} {} Z", 
            out_start_x, out_start_y, outer_r, outer_r, large_arc, sweep, out_end_x, out_end_y,
            in_end_x, in_end_y, inner_r, inner_r, large_arc, 1 - sweep, in_start_x, in_start_y)
    } else {
        let start_x = cx + outer_r * start_angle.cos();
        let start_y = cy + outer_r * start_angle.sin();
        let end_x = cx + outer_r * end_angle.cos();
        let end_y = cy + outer_r * end_angle.sin();
        let diff = (end_angle - start_angle).abs();
        let large_arc = if diff > PI { 1 } else { 0 };
        let sweep = if end_angle > start_angle { 1 } else { 0 };
        format!("M {} {} A {} {} 0 {} {} {} {} L {} {} Z", start_x, start_y, outer_r, outer_r, large_arc, sweep, end_x, end_y, cx, cy)
    }
}

pub fn catmull_rom_to_bezier(points: &[(f32, f32)]) -> String {
    if points.is_empty() { return String::new(); }
    if points.len() == 1 { return format!("M {} {}", points[0].0, points[0].1); }
    if points.len() == 2 { return format!("M {} {} L {} {}", points[0].0, points[0].1, points[1].0, points[1].1); }
    
    let mut path = format!("M {} {}", points[0].0, points[0].1);
    
    for i in 0..points.len() - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < points.len() { points[i + 2] } else { points[i + 1] };
        
        let cp1x = p1.0 + (p2.0 - p0.0) / 6.0;
        let cp1y = p1.1 + (p2.1 - p0.1) / 6.0;
        
        let cp2x = p2.0 - (p3.0 - p1.0) / 6.0;
        let cp2y = p2.1 - (p3.1 - p1.1) / 6.0;
        
        path.push_str(&format!(" C {} {} {} {} {} {}", cp1x, cp1y, cp2x, cp2y, p2.0, p2.1));
    }
    
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catmull_rom() {
        let path = catmull_rom_to_bezier(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]);
        assert!(path.starts_with("M 0 0 C"));
    }
}
