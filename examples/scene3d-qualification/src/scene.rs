use std::sync::OnceLock;

use fission::scene3d::*;

use crate::game::{BEACON_NODE, CARGO_BODY, PLAYER_BODY};

const BEACON_GLTF_SHA256: &str = env!("SCENE3D_BEACON_GLTF_SHA256");
const BEACON_TEXTURE_SHA256: &str = env!("SCENE3D_BEACON_TEXTURE_SHA256");

const GROUND_MATERIAL: ResourceId = ResourceId(1_000);
const PLAYER_MATERIAL: ResourceId = ResourceId(1_001);
const CARGO_MATERIAL: ResourceId = ResourceId(1_002);
const OBSTACLE_MATERIAL: ResourceId = ResourceId(1_003);

/// Imports the example's packaged static glTF once. Subsequent scene rebuilds
/// retain the same resource identities and revisions for renderer-side reuse.
pub fn imported_beacon() -> &'static ImportedGltf {
    static IMPORTED: OnceLock<ImportedGltf> = OnceLock::new();
    IMPORTED.get_or_init(|| {
        GltfImporter::import(
            "assets/beacon.gltf",
            include_bytes!("../assets/beacon.gltf"),
            GltfImportOptions {
                model_id: ModelId(1),
                first_mesh_id: 100,
                first_texture_id: 100,
                first_material_id: 200,
                first_asset_id: 100,
                revision: 1,
            },
            |uri| match uri {
                "beacon.ppm" => Ok(include_bytes!("../assets/beacon.ppm").to_vec()),
                other => Err(format!("unpackaged beacon dependency '{other}'")),
            },
        )
        .expect("the checked-in beacon glTF must remain valid")
    })
}

pub fn build_scene(player: Vec3, cargo: Vec3, success: bool) -> Scene3DIR {
    let imported = imported_beacon();
    let mut scene = Scene3DIR::new(SceneId::new(3_001), Viewport3D::new(960.0, 540.0));
    scene.assets = AssetBundle {
        format_version: AssetBundle::FORMAT_VERSION,
        id: "scene3d-qualification-assets-v1".into(),
        assets: vec![
            AssetDescriptor {
                id: AssetId::new(100),
                kind: AssetKind::Texture,
                source: "asset://assets/beacon.ppm".into(),
                sha256: BEACON_TEXTURE_SHA256.into(),
            },
            AssetDescriptor {
                id: AssetId::new(101),
                kind: AssetKind::Material,
                source: "asset://assets/beacon.gltf".into(),
                sha256: BEACON_GLTF_SHA256.into(),
            },
            AssetDescriptor {
                id: AssetId::new(103),
                kind: AssetKind::Model,
                source: "asset://assets/beacon.gltf".into(),
                sha256: BEACON_GLTF_SHA256.into(),
            },
        ],
    };
    scene.resources = imported.resources.clone();
    add_materials(&mut scene.resources, success);

    scene.camera = Camera3D::perspective(
        Vec3::new(player.x + 6.5, 5.8, player.z + 8.0),
        Vec3::new(player.x, 0.8, player.z - 2.3),
        52.0_f32.to_radians(),
    );
    scene.clear = Clear3D {
        color: Rgba::new(0.025, 0.055, 0.11, 1.0),
        depth: Some(1.0),
    };
    scene.lights = vec![
        Light3D::Ambient(AmbientLight3D {
            color: Rgba::new(0.28, 0.38, 0.62, 1.0),
            intensity: 0.32,
        }),
        Light3D::Directional(DirectionalLight3D {
            direction: Vec3::new(-0.6, -1.0, -0.4),
            color: Rgba::new(0.86, 0.92, 1.0, 1.0),
            intensity: 1.15,
        }),
        Light3D::Point(PointLight3D {
            position: Vec3::new(0.0, 3.2, -3.4),
            color: if success {
                Rgba::new(0.25, 1.0, 0.58, 1.0)
            } else {
                Rgba::new(0.15, 0.82, 1.0, 1.0)
            },
            intensity: 4.5,
            range: 11.0,
        }),
    ];

    scene.nodes = vec![
        primitive_node(
            NodeId::new(10),
            Vec3::new(0.0, -0.25, 0.0),
            Primitive3D::Cube {
                size: Vec3::new(12.0, 0.5, 14.0),
            },
            GROUND_MATERIAL,
        ),
        primitive_node(
            NodeId::new(11),
            Vec3::new(0.0, 0.75, 0.5),
            Primitive3D::Cube {
                size: Vec3::new(1.6, 1.5, 1.6),
            },
            OBSTACLE_MATERIAL,
        ),
        primitive_node(
            NodeId::new(12),
            Vec3::new(-4.8, 0.8, -0.8),
            Primitive3D::Cube {
                size: Vec3::new(0.7, 1.6, 5.0),
            },
            OBSTACLE_MATERIAL,
        ),
        primitive_node(
            NodeId::new(13),
            Vec3::new(4.8, 0.8, -0.8),
            Primitive3D::Cube {
                size: Vec3::new(0.7, 1.6, 5.0),
            },
            OBSTACLE_MATERIAL,
        ),
        primitive_node(
            NodeId::new(PLAYER_BODY.get()),
            player,
            Primitive3D::Sphere { radius: 0.48 },
            PLAYER_MATERIAL,
        ),
        primitive_node(
            NodeId::new(CARGO_BODY.get()),
            cargo,
            Primitive3D::Sphere { radius: 0.38 },
            CARGO_MATERIAL,
        ),
        Node3D {
            id: BEACON_NODE,
            parent: None,
            transform: Transform3 {
                translation: Vec3::new(0.0, 0.0, -4.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(1.15, 1.15, 1.15),
            },
            visible: true,
            content: NodeContent3D::Model {
                model: imported.model,
            },
            material: None,
            blend_order: 0,
            pickable: true,
        },
    ];
    scene
}

fn primitive_node(
    id: NodeId,
    translation: Vec3,
    primitive: Primitive3D,
    material: ResourceId,
) -> Node3D {
    Node3D {
        id,
        parent: None,
        transform: Transform3 {
            translation,
            ..Transform3::IDENTITY
        },
        visible: true,
        content: NodeContent3D::Primitive(primitive),
        material: Some(material),
        blend_order: 0,
        pickable: false,
    }
}

fn add_materials(resources: &mut Scene3DResources, success: bool) {
    resources.materials.extend([
        (
            GROUND_MATERIAL,
            material(Rgba::new(0.06, 0.13, 0.22, 1.0), 0.05, 0.88),
        ),
        (
            PLAYER_MATERIAL,
            material(
                if success {
                    Rgba::new(0.24, 0.95, 0.56, 1.0)
                } else {
                    Rgba::new(0.18, 0.68, 1.0, 1.0)
                },
                0.18,
                0.42,
            ),
        ),
        (
            CARGO_MATERIAL,
            material(Rgba::new(1.0, 0.58, 0.12, 1.0), 0.6, 0.28),
        ),
        (
            OBSTACLE_MATERIAL,
            material(Rgba::new(0.24, 0.3, 0.42, 1.0), 0.5, 0.72),
        ),
    ]);
}

fn material(base_color: Rgba, metallic: f32, roughness: f32) -> Material3D {
    Material3D {
        revision: 1,
        model: MaterialModel3D::MetallicRoughness {
            metallic,
            roughness,
        },
        base_color,
        ..Material3D::default()
    }
}
