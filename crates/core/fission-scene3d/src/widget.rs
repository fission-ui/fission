use fission_core::authoring::{IrBuilder, LowerWidget, LoweringContext};
use fission_core::ui::{Container, Widget};
use fission_core::ActionEnvelope;
use fission_ir::op::{EmbedKind, LayoutOp, Op};
use fission_ir::{
    ActionEntry, ActionTrigger, Role, SceneDimension, SceneTarget, Semantics, WidgetId,
};
use serde::{Deserialize, Serialize};

use crate::{PreparedScene3D, RenderCapabilities3D, Scene3DIR, Scene3DProcessor};

/// Type discriminator placed before every serialised 3D render packet.
pub const SCENE3D_EMBED_MAGIC: &[u8; 16] = b"fission.scene3d\0";

/// Complete renderer input carried by the Fission widget adapter.
///
/// The closed source IR remains present for resource resolution and picking;
/// the prepared value is the deterministic validation, transform, culling,
/// and draw plan for the declared default renderer capabilities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene3DRenderPacket {
    pub scene: Scene3DIR,
    pub prepared: PreparedScene3D,
}

impl Scene3DRenderPacket {
    pub fn new(scene: Scene3DIR) -> Self {
        let prepared = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
        Self { scene, prepared }
    }

    pub fn encode(&self) -> Result<Vec<u8>, bincode::Error> {
        let encoded = bincode::serialize(self)?;
        let mut payload = Vec::with_capacity(SCENE3D_EMBED_MAGIC.len() + encoded.len());
        payload.extend_from_slice(SCENE3D_EMBED_MAGIC);
        payload.extend_from_slice(&encoded);
        Ok(payload)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, String> {
        let Some(encoded) = payload.strip_prefix(SCENE3D_EMBED_MAGIC) else {
            return Err("payload is not a Fission 3D scene".into());
        };
        bincode::deserialize(encoded).map_err(|error| error.to_string())
    }
}

/// A retained 3D scene embedded in an ordinary Fission widget tree.
#[derive(Clone, Debug)]
pub struct Scene3D {
    pub scene: Scene3DIR,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub on_pick: Option<ActionEnvelope>,
    pub semantic_label: Option<String>,
}

impl Scene3D {
    pub fn new(scene: Scene3DIR) -> Self {
        Self {
            scene,
            width: None,
            height: None,
            on_pick: None,
            semantic_label: None,
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

    /// Dispatches a normal Fission action with [`fission_core::SceneInteraction`]
    /// when the viewport is activated. Pass its `viewport_point` to
    /// [`PreparedScene3D::pick_viewport`](crate::PreparedScene3D::pick_viewport)
    /// to resolve the stable scene node.
    pub fn on_pick(mut self, action: impl Into<ActionEnvelope>) -> Self {
        self.on_pick = Some(action.into());
        self
    }

    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }
}

impl From<Scene3D> for Widget {
    fn from(scene: Scene3D) -> Self {
        let mut container = Container::new(fission_core::authoring::custom_widget(
            "fission_scene3d::Scene3D",
            Scene3DLowerer {
                scene: scene.scene,
                on_pick: scene.on_pick,
                semantic_label: scene.semantic_label,
            },
        ));
        container = match scene.width {
            Some(width) => container.width(width),
            None => container.flex_grow(1.0),
        };
        container = match scene.height {
            Some(height) => container.height(height),
            None => container.flex_grow(1.0),
        };
        container.into()
    }
}

#[derive(Debug)]
struct Scene3DLowerer {
    scene: Scene3DIR,
    on_pick: Option<ActionEnvelope>,
    semantic_label: Option<String>,
}

impl LowerWidget for Scene3DLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> fission_ir::WidgetId {
        let viewport_id = WidgetId::explicit(&format!("fission.scene3d:{}", self.scene.id.get()));
        let node_id = WidgetId::derived(viewport_id.as_u128(), &[1]);
        let packet = Scene3DRenderPacket::new(self.scene.clone());
        let payload = packet.encode().unwrap_or_default();
        let viewport = self.scene.viewport.size;
        let viewport_origin = self.scene.viewport.origin;
        let widget_key = format!("fission_scene3d_scene_{}", self.scene.id.get());
        let embed = IrBuilder::new(
            node_id,
            fission_ir::Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Custom(payload),
                widget_id: fission_ir::WidgetId::explicit(&widget_key),
                width: Some(viewport.x),
                height: Some(viewport.y),
            }),
        )
        .build(cx);
        let Some(action) = &self.on_pick else {
            return embed;
        };
        let semantics = Semantics {
            role: Role::Image,
            label: self.semantic_label.clone(),
            identifier: Some(format!("scene3d:{}", self.scene.id.get())),
            actions: fission_ir::ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: action.id.as_u128(),
                    payload_data: Some(action.payload.clone()),
                }],
            },
            focusable: true,
            scene_target: Some(SceneTarget {
                viewport_id: viewport_id.as_u128(),
                scene_id: self.scene.id.get(),
                node_id: None,
                instance: None,
                dimension: SceneDimension::Three,
                viewport_origin: [viewport_origin.x, viewport_origin.y],
                view_to_scene: None,
            }),
            ..Default::default()
        };
        let mut root = IrBuilder::new(viewport_id, Op::Semantics(semantics));
        root.add_child(embed);
        root.build(cx)
    }
}

#[cfg(test)]
mod tests {
    use fission_core::{ActionEnvelope, ActionId};
    use fission_scene::SceneId;

    use super::*;
    use crate::Viewport3D;

    #[test]
    fn packets_are_typed_versioned_and_round_trip() {
        let packet =
            Scene3DRenderPacket::new(Scene3DIR::new(SceneId(5), Viewport3D::new(320.0, 180.0)));
        let encoded = packet.encode().unwrap();

        assert!(encoded.starts_with(SCENE3D_EMBED_MAGIC));
        assert_eq!(Scene3DRenderPacket::decode(&encoded).unwrap(), packet);
        assert_eq!(packet.scene.format_version, crate::SCENE3D_FORMAT_VERSION);
        assert!(Scene3DRenderPacket::decode(b"not-a-scene").is_err());
    }

    #[test]
    fn picking_semantics_use_the_declared_viewport_origin() {
        let mut scene = Scene3DIR::new(SceneId(7), Viewport3D::new(320.0, 180.0));
        scene.viewport.origin = fission_scene::Vec2::new(12.0, 18.0);
        let widget: Widget = Scene3D::new(scene)
            .on_pick(ActionEnvelope {
                id: ActionId::from_name("scene3d-test-pick"),
                payload: vec![],
            })
            .into();

        let ir = fission_core::internal::lower_widget_to_ir(&widget);
        let target = ir
            .nodes
            .values()
            .find_map(|node| match &node.op {
                Op::Semantics(semantics) => semantics.scene_target.as_ref(),
                _ => None,
            })
            .expect("scene picking semantics");

        assert_eq!(target.viewport_origin, [12.0, 18.0]);
    }
}
