use fission_core::authoring::{custom_widget, IrBuilder, LowerWidget, LoweringContext};
use fission_core::internal::wrap_zstack_child;
use fission_core::ui::Widget;
use fission_ir::op::{
    BlendMode, BoxStyle, Color, CompositeScalar, CompositeStyle, Fill, ImageAlignment,
    ImageBatchInstance, ImageFit, ImageRequest, ImageSampling, ImageSource, LayoutOp, Length,
    LineCap, LineJoin, Op, PaintOp, Stroke,
};
use fission_ir::semantics::{ActionTrigger, Role, SceneDimension, SceneTarget};
use fission_ir::{ActionEntry, Semantics, StructuralOp, WidgetId};
use fission_scene::{Bounds2, PresentationId, Rgba, Vec2};
use serde::{Deserialize, Serialize};

use crate::{
    ActionBinding2D, Affine2, BlendMode2D, Clip2D, DrawCommand2D, DrawMetadata2D, ImageSampling2D,
    PathCommand2D, PreparedClip2D, PreparedScene2D, Rect2D, Scene2DIR, Scene2DProcessor,
    SemanticRole2D,
};

pub const SCENE2D_EMBED_MAGIC: &[u8; 16] = b"fission.scene2d\0";

/// Serializable processed scene for recording and inspection tooling. Visible
/// scenes lower to ordinary Fission IR rather than embedding this packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene2DRenderPacket {
    pub scene: Scene2DIR,
    pub prepared: PreparedScene2D,
}

impl Scene2DRenderPacket {
    pub fn new(scene: Scene2DIR) -> Self {
        let prepared = Scene2DProcessor::prepare(&scene);
        Self { scene, prepared }
    }

    pub fn encode(&self) -> Result<Vec<u8>, bincode::Error> {
        let mut payload = SCENE2D_EMBED_MAGIC.to_vec();
        payload.extend(bincode::serialize(self)?);
        Ok(payload)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, String> {
        let data = payload
            .strip_prefix(SCENE2D_EMBED_MAGIC)
            .ok_or_else(|| "payload is not a Fission 2D scene".to_string())?;
        bincode::deserialize(data).map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct Scene2D {
    pub scene: Scene2DIR,
    pub presentation_id: PresentationId,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Scene2D {
    pub fn new(scene: Scene2DIR) -> Self {
        let presentation_id = PresentationId::new(scene.id.get());
        Self {
            scene,
            presentation_id,
            width: None,
            height: None,
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
    pub fn presentation_id(mut self, presentation: PresentationId) -> Self {
        self.presentation_id = presentation;
        self
    }
}

impl From<Scene2D> for Widget {
    fn from(value: Scene2D) -> Self {
        let size = value.scene.viewport.rect.size;
        custom_widget(
            "fission_scene2d::Scene2D",
            Scene2DLowerer {
                scene: value.scene,
                presentation: value.presentation_id,
                width: value.width.unwrap_or(size.x),
                height: value.height.unwrap_or(size.y),
            },
        )
    }
}

#[derive(Debug)]
struct Scene2DLowerer {
    scene: Scene2DIR,
    presentation: PresentationId,
    width: f32,
    height: f32,
}

impl LowerWidget for Scene2DLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let prepared = Scene2DProcessor::prepare(&self.scene);
        let presentation = self.presentation.get();
        let root_id = sid(presentation, 0, "root");
        let mut stack = IrBuilder::new(
            WidgetId::derived(root_id.as_u128(), &[1]),
            Op::Layout(LayoutOp::ZStack),
        );
        for draw in &prepared.draws {
            let visual = lower_draw(cx, &self.scene, presentation, draw, self.width, self.height);
            stack.add_child(wrap_zstack_child(cx, visual));
            if let Some(interaction) = draw
                .metadata()
                .interaction
                .as_ref()
                .filter(|i| i.is_interactive())
            {
                let hit_regions = match draw {
                    DrawCommand2D::ImageBatch { instances, .. } => instances
                        .iter()
                        .enumerate()
                        .map(|(index, instance)| (instance.view_bounds, Some(index as u32)))
                        .collect::<Vec<_>>(),
                    DrawCommand2D::SpriteBatch { instances, .. } => instances
                        .iter()
                        .enumerate()
                        .map(|(index, instance)| (instance.view_bounds, Some(index as u32)))
                        .collect::<Vec<_>>(),
                    _ => vec![(draw.metadata().view_bounds, None)],
                };
                for (bounds, instance) in hit_regions {
                    let semantics = lower_semantics(
                        cx,
                        self.scene.id.get(),
                        presentation,
                        root_id,
                        draw.metadata(),
                        bounds,
                        instance,
                        prepared.view_to_scene,
                        interaction,
                        self.scene.viewport.rect.origin,
                        self.scene.viewport.rect.size,
                    );
                    stack.add_child(wrap_zstack_child(cx, semantics));
                }
            }
        }
        let stack = stack.build(cx);
        let mut viewport_clip = IrBuilder::new(
            WidgetId::derived(root_id.as_u128(), &[2]),
            Op::Layout(LayoutOp::Clip {
                path: Some(format!(
                    "M0 0 L{} 0 L{} {} L0 {} Z",
                    self.width, self.width, self.height, self.height
                )),
            }),
        );
        viewport_clip.add_child(stack);
        let viewport_clip = viewport_clip.build(cx);
        let mut root = IrBuilder::new(root_id, fixed_box(self.width, self.height));
        root.add_child(viewport_clip);
        root.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(
            sid(self.presentation.get(), 0, "root").as_u128(),
            &[0x57_4944],
        ))
    }
}

fn lower_draw(
    cx: &mut LoweringContext,
    scene: &Scene2DIR,
    presentation: u64,
    draw: &DrawCommand2D,
    width: f32,
    height: f32,
) -> WidgetId {
    let origin = scene.viewport.rect.origin;
    match draw {
        DrawCommand2D::ImageBatch {
            metadata,
            image,
            sampling,
            instances,
        } => lower_batch(
            cx,
            scene,
            presentation,
            metadata,
            *image,
            *sampling,
            instances.iter().map(|item| ImageBatchInstance {
                transform: affine6(item.view_transform, origin),
                destination: rect4(item.destination),
                source: item.source.map(|source| rect4(source.rect)),
                tint: color(item.tint),
                opacity: item.opacity,
            }),
            width,
            height,
        ),
        DrawCommand2D::SpriteBatch {
            metadata,
            image,
            sampling,
            instances,
        } => lower_batch(
            cx,
            scene,
            presentation,
            metadata,
            *image,
            *sampling,
            instances.iter().map(|item| ImageBatchInstance {
                transform: affine6(item.view_transform, origin),
                destination: rect4(item.destination),
                source: Some(rect4(item.source.rect)),
                tint: color(item.tint),
                opacity: item.opacity,
            }),
            width,
            height,
        ),
        DrawCommand2D::Rectangle {
            metadata,
            style,
            corner_radius,
            ..
        } => lower_paint(
            cx,
            scene,
            presentation,
            metadata,
            PaintOp::DrawRect {
                fill: style.fill.map(|v| Fill::Solid(color(v.color))),
                stroke: style.stroke.map(stroke),
                corner_radius: *corner_radius,
                shadow: None,
                corner_radii: None,
                border_sides: None,
            },
        ),
        DrawCommand2D::Path {
            metadata,
            path,
            style,
        } => {
            let resource = &scene.resources.paths[path];
            lower_paint(
                cx,
                scene,
                presentation,
                metadata,
                PaintOp::DrawPath {
                    path: svg(&resource.commands, Affine2::IDENTITY, resource.bounds.min),
                    fill: style.fill.map(|v| Fill::Solid(color(v.color))),
                    stroke: style.stroke.map(stroke),
                    view_box: Some([
                        resource.bounds.max.x - resource.bounds.min.x,
                        resource.bounds.max.y - resource.bounds.min.y,
                    ]),
                },
            )
        }
        DrawCommand2D::Text { metadata, text } => {
            let text = &scene.resources.text[text];
            lower_paint(
                cx,
                scene,
                presentation,
                metadata,
                PaintOp::DrawText {
                    text: text.text.clone(),
                    size: text.font_size,
                    color: color(text.color),
                    underline: false,
                    locale: None,
                    wrap: false,
                    caret_index: None,
                    caret_color: None,
                    caret_width: None,
                    caret_height: None,
                    caret_radius: None,
                    paragraph_style: None,
                },
            )
        }
        DrawCommand2D::Image {
            metadata,
            image,
            destination,
            source,
            sampling,
            tint,
        } if source.is_some() || *tint != Rgba::WHITE => lower_paint(
            cx,
            scene,
            presentation,
            metadata,
            PaintOp::DrawImageBatch {
                request: request(scene, *image),
                sampling: match sampling {
                    ImageSampling2D::Nearest => ImageSampling::Nearest,
                    ImageSampling2D::Linear => ImageSampling::Linear,
                },
                instances: vec![ImageBatchInstance {
                    transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    destination: [0.0, 0.0, destination.size.x, destination.size.y],
                    source: source.map(|value| rect4(value.rect)),
                    tint: color(*tint),
                    opacity: 1.0,
                }],
            },
        ),
        DrawCommand2D::Image {
            metadata, image, ..
        } => lower_paint(
            cx,
            scene,
            presentation,
            metadata,
            PaintOp::DrawImage {
                request: request(scene, *image),
                fit: ImageFit::Fill,
                alignment: ImageAlignment::Center,
            },
        ),
    }
}

fn lower_paint(
    cx: &mut LoweringContext,
    scene: &Scene2DIR,
    presentation: u64,
    meta: &DrawMetadata2D,
    paint: PaintOp,
) -> WidgetId {
    let base = sid(presentation, meta.node.get(), "visual");
    let paint = IrBuilder::new(WidgetId::derived(base.as_u128(), &[1]), Op::Paint(paint)).build(cx);
    let size = size(meta.local_bounds);
    let mut box_node = IrBuilder::new(
        WidgetId::derived(base.as_u128(), &[2]),
        fixed_box(size.x, size.y),
    );
    box_node.add_child(paint);
    let mut child = box_node.build(cx);
    for (index, clip) in meta.clips.iter().enumerate() {
        let Some(inverse) = meta.view_transform.inverse() else {
            continue;
        };
        let path = clip_svg(
            scene,
            clip,
            inverse.then(clip.view_transform),
            meta.local_bounds.min,
        );
        let mut node = IrBuilder::new(
            WidgetId::derived(base.as_u128(), &[3, index as u32]),
            Op::Layout(LayoutOp::Clip { path: Some(path) }),
        );
        node.add_child(child);
        child = node.build(cx);
    }
    let at_min = meta.view_transform.transform_point(meta.local_bounds.min);
    let transform = Affine2 {
        tx: at_min.x - meta.view_bounds.min.x,
        ty: at_min.y - meta.view_bounds.min.y,
        ..meta.view_transform
    };
    let mut node = IrBuilder::new(
        WidgetId::derived(base.as_u128(), &[4]),
        Op::Layout(LayoutOp::Transform {
            transform: matrix4(transform),
        }),
    )
    .composite(CompositeStyle {
        opacity: Some(CompositeScalar::new(meta.opacity)),
        blend_mode: blend(meta.blend_mode),
        ..Default::default()
    });
    node.add_child(child);
    let transformed = node.build(cx);
    child = inert(cx, transformed);
    positioned(
        cx,
        WidgetId::derived(base.as_u128(), &[5]),
        meta.view_bounds.min.x - scene.viewport.rect.origin.x,
        meta.view_bounds.min.y - scene.viewport.rect.origin.y,
        None,
        None,
        child,
    )
}

fn lower_batch(
    cx: &mut LoweringContext,
    scene: &Scene2DIR,
    presentation: u64,
    meta: &DrawMetadata2D,
    image: crate::ImageHandle2D,
    sampling: ImageSampling2D,
    instances: impl IntoIterator<Item = ImageBatchInstance>,
    width: f32,
    height: f32,
) -> WidgetId {
    let base = sid(presentation, meta.node.get(), "batch");
    let paint = IrBuilder::new(
        WidgetId::derived(base.as_u128(), &[1]),
        Op::Paint(PaintOp::DrawImageBatch {
            request: request(scene, image),
            sampling: match sampling {
                ImageSampling2D::Nearest => ImageSampling::Nearest,
                ImageSampling2D::Linear => ImageSampling::Linear,
            },
            instances: instances.into_iter().collect(),
        }),
    )
    .build(cx);
    let mut child = paint;
    for (index, clip) in meta.clips.iter().enumerate() {
        let path = clip_svg(scene, clip, clip.view_transform, scene.viewport.rect.origin);
        let mut node = IrBuilder::new(
            WidgetId::derived(base.as_u128(), &[2, index as u32]),
            Op::Layout(LayoutOp::Clip { path: Some(path) }),
        );
        node.add_child(child);
        child = node.build(cx);
    }
    let mut node = IrBuilder::new(
        WidgetId::derived(base.as_u128(), &[3]),
        fixed_box(width, height),
    )
    .composite(CompositeStyle {
        blend_mode: blend(meta.blend_mode),
        ..Default::default()
    });
    let child = inert(cx, child);
    node.add_child(child);
    node.build(cx)
}

fn lower_semantics(
    cx: &mut LoweringContext,
    scene: u64,
    presentation: u64,
    viewport_id: WidgetId,
    meta: &DrawMetadata2D,
    bounds: Bounds2,
    instance: Option<u32>,
    view_to_scene: Affine2,
    interaction: &crate::Interaction2D,
    origin: Vec2,
    viewport_size: Vec2,
) -> WidgetId {
    let suffix = instance.map_or_else(
        || "interaction".to_owned(),
        |index| format!("interaction:{index}"),
    );
    let base = sid(presentation, meta.node.get(), &suffix);
    let dimensions = size(bounds);
    let child = IrBuilder::new(
        WidgetId::derived(base.as_u128(), &[1]),
        fixed_box(dimensions.x, dimensions.y),
    )
    .build(cx);
    let mut value = Semantics {
        role: match interaction.semantic_role.unwrap_or(SemanticRole2D::Generic) {
            SemanticRole2D::Button => Role::Button,
            SemanticRole2D::Image => Role::Image,
            SemanticRole2D::Generic => Role::Generic,
        },
        label: interaction.semantic_label.clone(),
        identifier: Some(instance.map_or_else(
            || format!("scene2d:{presentation}:node:{}", meta.node.get()),
            |index| {
                format!(
                    "scene2d:{presentation}:node:{}:instance:{index}",
                    meta.node.get()
                )
            },
        )),
        focusable: interaction.tap.is_some(),
        draggable: interaction.drag.is_some(),
        scene_target: Some(SceneTarget {
            viewport_id: viewport_id.as_u128(),
            scene_id: scene,
            presentation_id: presentation,
            node_id: Some(meta.node.get()),
            instance,
            dimension: SceneDimension::Two,
            viewport_origin: [origin.x, origin.y],
            viewport_size: [viewport_size.x, viewport_size.y],
            view_to_scene: Some([
                view_to_scene.m11,
                view_to_scene.m12,
                view_to_scene.m21,
                view_to_scene.m22,
                view_to_scene.tx,
                view_to_scene.ty,
            ]),
        }),
        ..Default::default()
    };
    action(&mut value, ActionTrigger::Default, interaction.tap.as_ref());
    action(
        &mut value,
        ActionTrigger::LongPress,
        interaction.long_press.as_ref(),
    );
    if let Some(drag) = &interaction.drag {
        action(&mut value, ActionTrigger::DragStart, drag.start.as_ref());
        action(&mut value, ActionTrigger::DragUpdate, drag.update.as_ref());
        action(&mut value, ActionTrigger::DragEnd, drag.end.as_ref());
        action(&mut value, ActionTrigger::DragCancel, drag.cancel.as_ref());
    }
    let mut node = IrBuilder::new(base, Op::Semantics(value));
    node.add_child(child);
    let node = node.build(cx);
    positioned(
        cx,
        WidgetId::derived(base.as_u128(), &[2]),
        bounds.min.x - origin.x,
        bounds.min.y - origin.y,
        Some(dimensions.x),
        Some(dimensions.y),
        node,
    )
}

fn action(semantics: &mut Semantics, trigger: ActionTrigger, binding: Option<&ActionBinding2D>) {
    if let Some(binding) = binding {
        semantics.actions.entries.push(ActionEntry {
            trigger,
            action_id: binding.action.0,
            payload_data: Some(binding.payload.clone()),
        });
    }
}

fn inert(cx: &mut LoweringContext, child: WidgetId) -> WidgetId {
    let mut node = IrBuilder::new(
        WidgetId::derived(child.as_u128(), &[0x2D]),
        Op::Structural(StructuralOp::PointerTransparent {
            stable_hash: child.as_u128() as u64,
        }),
    );
    node.add_child(child);
    node.build(cx)
}

fn positioned(
    cx: &mut LoweringContext,
    id: WidgetId,
    left: f32,
    top: f32,
    width: Option<f32>,
    height: Option<f32>,
    child: WidgetId,
) -> WidgetId {
    let mut node = IrBuilder::new(
        id,
        Op::Layout(LayoutOp::Positioned {
            left: Some(left),
            top: Some(top),
            right: None,
            bottom: None,
            width,
            height,
        }),
    );
    node.add_child(child);
    node.build(cx)
}

fn fixed_box(width: f32, height: f32) -> Op {
    Op::Layout(LayoutOp::StyledBox {
        style: BoxStyle {
            width: Some(Length::points(width.max(0.0))),
            height: Some(Length::points(height.max(0.0))),
            ..Default::default()
        },
        flex_grow: 0.0,
        flex_shrink: 0.0,
    })
}

fn request(scene: &Scene2DIR, image: crate::ImageHandle2D) -> ImageRequest {
    let asset = scene
        .assets
        .assets
        .iter()
        .find(|asset| asset.id == image.id())
        .expect("prepared image asset");
    ImageRequest {
        source: ImageSource::Asset {
            path: asset.source.clone(),
        },
        ..Default::default()
    }
}

fn clip_svg(scene: &Scene2DIR, clip: &PreparedClip2D, transform: Affine2, offset: Vec2) -> String {
    match &clip.shape {
        Clip2D::Rectangle { rect } => rect_svg(*rect, transform, offset),
        Clip2D::Path { path } => svg(&scene.resources.paths[path].commands, transform, offset),
    }
}

fn rect_svg(rect: Rect2D, transform: Affine2, offset: Vec2) -> String {
    let b = rect.bounds();
    let p = [
        b.min,
        Vec2::new(b.max.x, b.min.y),
        b.max,
        Vec2::new(b.min.x, b.max.y),
    ]
    .map(|v| shift(transform.transform_point(v), offset));
    format!(
        "M{} {} L{} {} L{} {} L{} {} Z",
        p[0].x, p[0].y, p[1].x, p[1].y, p[2].x, p[2].y, p[3].x, p[3].y
    )
}

fn svg(commands: &[PathCommand2D], transform: Affine2, offset: Vec2) -> String {
    let point = |v| shift(transform.transform_point(v), offset);
    let mut output = String::new();
    for command in commands {
        match command {
            PathCommand2D::MoveTo { point: v } => {
                let p = point(*v);
                output += &format!("M{} {} ", p.x, p.y);
            }
            PathCommand2D::LineTo { point: v } => {
                let p = point(*v);
                output += &format!("L{} {} ", p.x, p.y);
            }
            PathCommand2D::QuadraticTo { control, point: v } => {
                let c = point(*control);
                let p = point(*v);
                output += &format!("Q{} {} {} {} ", c.x, c.y, p.x, p.y);
            }
            PathCommand2D::CubicTo {
                control1,
                control2,
                point: v,
            } => {
                let a = point(*control1);
                let b = point(*control2);
                let p = point(*v);
                output += &format!("C{} {} {} {} {} {} ", a.x, a.y, b.x, b.y, p.x, p.y);
            }
            PathCommand2D::Close => output += "Z ",
        }
    }
    output
}

fn shift(point: Vec2, offset: Vec2) -> Vec2 {
    Vec2::new(point.x - offset.x, point.y - offset.y)
}
fn sid(scene: u64, node: u64, suffix: &str) -> WidgetId {
    WidgetId::explicit(&format!("fission.scene2d:{scene}:{node}:{suffix}"))
}
fn size(bounds: Bounds2) -> Vec2 {
    Vec2::new(
        (bounds.max.x - bounds.min.x).max(0.0),
        (bounds.max.y - bounds.min.y).max(0.0),
    )
}
fn affine6(v: Affine2, origin: Vec2) -> [f32; 6] {
    [v.m11, v.m12, v.m21, v.m22, v.tx - origin.x, v.ty - origin.y]
}
fn matrix4(v: Affine2) -> [f32; 16] {
    [
        v.m11, v.m12, 0., 0., v.m21, v.m22, 0., 0., 0., 0., 1., 0., v.tx, v.ty, 0., 1.,
    ]
}
fn rect4(v: Rect2D) -> [f32; 4] {
    [v.origin.x, v.origin.y, v.size.x, v.size.y]
}
fn color(v: Rgba) -> Color {
    let c = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Color {
        r: c(v.red),
        g: c(v.green),
        b: c(v.blue),
        a: c(v.alpha),
    }
}
fn stroke(v: crate::Stroke2D) -> Stroke {
    Stroke {
        fill: Fill::Solid(color(v.color)),
        width: v.width,
        dash_array: None,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        dash_offset: 0.,
        trim: None,
    }
}
fn blend(v: BlendMode2D) -> BlendMode {
    match v {
        BlendMode2D::Normal => BlendMode::Normal,
        BlendMode2D::Multiply => BlendMode::Multiply,
        BlendMode2D::Screen => BlendMode::Screen,
        BlendMode2D::Add => BlendMode::Plus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node2D, Viewport2D};
    use fission_scene::{NodeId, SceneDiagnostic, SceneId};
    #[test]
    fn packets_round_trip() {
        let mut scene = Scene2DIR::new(SceneId(5), Viewport2D::new(320., 180.));
        scene.nodes.push(Node2D::group(NodeId::new(9)));
        let mut packet = Scene2DRenderPacket::new(scene);
        packet
            .prepared
            .diagnostics
            .push(SceneDiagnostic::error("round-trip", "diagnostic payload"));
        let bytes = packet.encode().unwrap();
        assert_eq!(Scene2DRenderPacket::decode(&bytes).unwrap(), packet);
    }
}
