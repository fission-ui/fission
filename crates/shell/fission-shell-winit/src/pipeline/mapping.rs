use super::*;

impl SnapshotProvider for Pipeline {
    fn snapshot(&self, kind: SnapshotKind) -> Option<SnapshotBlob> {
        match kind {
            SnapshotKind::Layout => self.last_snapshot.as_ref().and_then(|snap| {
                serde_json::to_string_pretty(snap)
                    .ok()
                    .map(|json| SnapshotBlob { kind, json })
            }),
        }
    }
}

pub(super) fn map_fill(f: &fission_ir::op::Fill) -> Fill {
    match f {
        fission_ir::op::Fill::Solid(c) => Fill::Solid(RenderColor {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }),
        fission_ir::op::Fill::LinearGradient { start, end, stops } => Fill::LinearGradient {
            start: *start,
            end: *end,
            stops: stops
                .iter()
                .map(|(o, c)| {
                    (
                        *o,
                        RenderColor {
                            r: c.r,
                            g: c.g,
                            b: c.b,
                            a: c.a,
                        },
                    )
                })
                .collect(),
        },
        fission_ir::op::Fill::RadialGradient {
            center,
            radius,
            stops,
        } => Fill::RadialGradient {
            center: *center,
            radius: *radius,
            stops: stops
                .iter()
                .map(|(o, c)| {
                    (
                        *o,
                        RenderColor {
                            r: c.r,
                            g: c.g,
                            b: c.b,
                            a: c.a,
                        },
                    )
                })
                .collect(),
        },
    }
}

pub(super) fn map_stroke(s: &fission_ir::op::Stroke) -> Stroke {
    Stroke {
        fill: map_fill(&s.fill),
        width: s.width,
        dash_array: s.dash_array.clone(),
        line_cap: match s.line_cap {
            fission_ir::op::LineCap::Butt => fission_render::LineCap::Butt,
            fission_ir::op::LineCap::Round => fission_render::LineCap::Round,
            fission_ir::op::LineCap::Square => fission_render::LineCap::Square,
        },
        line_join: match s.line_join {
            fission_ir::op::LineJoin::Miter => fission_render::LineJoin::Miter,
            fission_ir::op::LineJoin::Round => fission_render::LineJoin::Round,
            fission_ir::op::LineJoin::Bevel => fission_render::LineJoin::Bevel,
        },
    }
}

pub(super) fn translate_rect(rect: LayoutRect, offset: LayoutPoint) -> LayoutRect {
    LayoutRect {
        origin: LayoutPoint::new(rect.origin.x + offset.x, rect.origin.y + offset.y),
        size: rect.size,
    }
}
