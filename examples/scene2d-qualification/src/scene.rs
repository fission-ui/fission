use fission::core::Action;
use fission::scene2d::*;

use crate::game::{
    Direction, MovePlayer, QualificationGame, BEACON_POSITION, OBSTACLE_CENTER, PLAYER_RADIUS,
    VIEWPORT_SIZE, WORLD_SIZE,
};

const SCENE_ID: SceneId = SceneId::new(1);
const SHEET_ASSET: AssetId = AssetId::new(1);
const SHEET_RESOURCE: ResourceId = ResourceId(1);
const ROUTE_RESOURCE: ResourceId = ResourceId(2);
const STATUS_RESOURCE: ResourceId = ResourceId(3);
const SHEET_SHA256: &str = "c8f7e002bc807665ca7f456a794be2a785a27f61d1244c0d23a7ab3d0bee1d6f";

const PLAYER_NODE: NodeId = NodeId::new(10);
const BEACON_NODE: NodeId = NodeId::new(11);
const OBSTACLE_NODE: NodeId = NodeId::new(12);
const ROUTE_NODE: NodeId = NodeId::new(13);
const STATUS_NODE: NodeId = NodeId::new(14);
pub const CONTROL_UP: u64 = 20;
pub const CONTROL_DOWN: u64 = 21;
pub const CONTROL_LEFT: u64 = 22;
pub const CONTROL_RIGHT: u64 = 23;

pub fn build_scene(game: &QualificationGame) -> Scene2DIR {
    let mut scene = Scene2DIR::new(SCENE_ID, Viewport2D::new(VIEWPORT_SIZE.x, VIEWPORT_SIZE.y));
    scene.camera.center = camera_center(game.player);
    scene.camera.zoom = 1.0;
    scene.assets = AssetBundle {
        format_version: AssetBundle::FORMAT_VERSION,
        id: "beacon-run-assets-v1".into(),
        assets: vec![AssetDescriptor {
            id: SHEET_ASSET,
            kind: AssetKind::Image,
            source: "assets/scout-sheet.svg".into(),
            sha256: SHEET_SHA256.into(),
        }],
    };
    add_resources(&mut scene, game);
    scene.nodes.push(rectangle(
        1,
        Rect2D::new(Vec2::ZERO, WORLD_SIZE),
        Rgba::new(0.035, 0.063, 0.11, 1.0),
        0,
    ));
    scene.nodes.push(Node2D {
        id: ROUTE_NODE,
        parent: None,
        transform: Transform2::IDENTITY,
        visible: true,
        opacity: 0.9,
        layer: 1,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        content: NodeContent2D::Path {
            path: ROUTE_RESOURCE,
            style: PathStyle2D {
                fill: None,
                stroke: Some(Stroke2D {
                    color: Rgba::new(0.18, 0.52, 0.96, 0.8),
                    width: 5.0,
                }),
            },
        },
        interaction: None,
    });
    let mut obstacle = rectangle(
        OBSTACLE_NODE.get(),
        Rect2D::new(Vec2::new(-92.0, -132.0), Vec2::new(184.0, 264.0)),
        Rgba::new(0.16, 0.2, 0.28, 1.0),
        4,
    );
    obstacle.transform.translation = OBSTACLE_CENTER;
    scene.nodes.push(obstacle);

    scene
        .nodes
        .push(sprite_node(PLAYER_NODE, game.player, 0, None, 10));
    if !game.beacon_collected {
        scene
            .nodes
            .push(sprite_node(BEACON_NODE, BEACON_POSITION, 1, None, 9));
    }
    add_status(&mut scene, game);
    add_controls(&mut scene, game.player);
    scene
}

fn add_resources(scene: &mut Scene2DIR, game: &QualificationGame) {
    scene.resources.sprite_sheets.insert(
        SHEET_RESOURCE,
        SpriteSheet2D {
            image: ImageHandle2D::new(SHEET_ASSET),
            frames: vec![
                SpriteFrame2D {
                    source: ImageSource2D {
                        rect: Rect2D::new(Vec2::ZERO, Vec2::new(64.0, 64.0)),
                    },
                    size: Vec2::new(52.0, 52.0),
                    pivot: Vec2::new(26.0, 26.0),
                },
                SpriteFrame2D {
                    source: ImageSource2D {
                        rect: Rect2D::new(Vec2::new(64.0, 0.0), Vec2::new(64.0, 64.0)),
                    },
                    size: Vec2::new(52.0, 52.0),
                    pivot: Vec2::new(26.0, 26.0),
                },
            ],
            sampling: ImageSampling2D::Linear,
        },
    );
    scene.resources.paths.insert(
        ROUTE_RESOURCE,
        PathResource2D {
            commands: vec![
                PathCommand2D::MoveTo {
                    point: Vec2::new(96.0, 360.0),
                },
                PathCommand2D::CubicTo {
                    control1: Vec2::new(300.0, 80.0),
                    control2: Vec2::new(790.0, 80.0),
                    point: BEACON_POSITION,
                },
            ],
            bounds: Bounds2::new(Vec2::new(96.0, 80.0), BEACON_POSITION),
        },
    );
    let label = if game.beacon_collected {
        format!("Beacon secured in {} moves", game.moves)
    } else {
        "Reach the beacon — the direct path is blocked".into()
    };
    scene.resources.text.insert(
        STATUS_RESOURCE,
        TextResource2D {
            text: label,
            font_key: "system-ui".into(),
            font_size: 18.0,
            glyphs: Vec::new(),
            bounds: Bounds2::new(Vec2::ZERO, Vec2::new(430.0, 28.0)),
            color: Rgba::WHITE,
        },
    );
}

fn add_status(scene: &mut Scene2DIR, game: &QualificationGame) {
    let camera = camera_center(game.player);
    scene.nodes.push(Node2D {
        id: STATUS_NODE,
        parent: None,
        transform: Transform2 {
            translation: Vec2::new(
                camera.x - VIEWPORT_SIZE.x / 2.0 + 20.0,
                camera.y - VIEWPORT_SIZE.y / 2.0 + 18.0,
            ),
            ..Transform2::IDENTITY
        },
        visible: true,
        opacity: 1.0,
        layer: 20,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        content: NodeContent2D::Text {
            text: STATUS_RESOURCE,
        },
        interaction: None,
    });
}

fn add_controls(scene: &mut Scene2DIR, player: Vec2) {
    let camera = camera_center(player);
    let origin = Vec2::new(
        camera.x + VIEWPORT_SIZE.x / 2.0 - 132.0,
        camera.y + VIEWPORT_SIZE.y / 2.0 - 112.0,
    );
    for (id, direction, offset, label) in [
        (CONTROL_UP, Direction::Up, Vec2::new(44.0, 0.0), "Move up"),
        (
            CONTROL_DOWN,
            Direction::Down,
            Vec2::new(44.0, 44.0),
            "Move down",
        ),
        (
            CONTROL_LEFT,
            Direction::Left,
            Vec2::new(0.0, 44.0),
            "Move left",
        ),
        (
            CONTROL_RIGHT,
            Direction::Right,
            Vec2::new(88.0, 44.0),
            "Move right",
        ),
    ] {
        let mut node = rectangle(
            id,
            Rect2D::new(Vec2::ZERO, Vec2::new(40.0, 40.0)),
            Rgba::new(0.11, 0.22, 0.38, 0.92),
            30,
        );
        node.transform.translation = Vec2::new(origin.x + offset.x, origin.y + offset.y);
        node.interaction = Some(interaction(direction, label));
        scene.nodes.push(node);
    }
}

fn interaction(direction: Direction, label: &str) -> Interaction2D {
    let action = MovePlayer(direction);
    Interaction2D {
        tap: Some(ActionBinding2D::new(
            ActionToken(MovePlayer::static_id().as_u128()),
            action.encode(),
        )),
        semantic_label: Some(label.into()),
        semantic_role: Some(SemanticRole2D::Button),
        ..Default::default()
    }
}

fn sprite_node(
    id: NodeId,
    translation: Vec2,
    frame: u32,
    interaction: Option<Interaction2D>,
    layer: i32,
) -> Node2D {
    Node2D {
        id,
        parent: None,
        transform: Transform2 {
            translation,
            pivot: Vec2::new(PLAYER_RADIUS, PLAYER_RADIUS),
            ..Transform2::IDENTITY
        },
        visible: true,
        opacity: 1.0,
        layer,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        content: NodeContent2D::Sprite(Sprite2D {
            sheet: SHEET_RESOURCE,
            frame,
            tint: Rgba::WHITE,
        }),
        interaction,
    }
}

fn rectangle(id: u64, rect: Rect2D, color: Rgba, layer: i32) -> Node2D {
    Node2D {
        id: NodeId::new(id),
        parent: None,
        transform: Transform2::IDENTITY,
        visible: true,
        opacity: 1.0,
        layer,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        content: NodeContent2D::Rectangle {
            rect,
            style: PathStyle2D {
                fill: Some(Fill2D { color }),
                stroke: Some(Stroke2D {
                    color: Rgba::new(0.28, 0.38, 0.52, 1.0),
                    width: 2.0,
                }),
            },
            corner_radius: 8.0,
        },
        interaction: None,
    }
}

fn camera_center(player: Vec2) -> Vec2 {
    Vec2::new(
        player
            .x
            .clamp(VIEWPORT_SIZE.x / 2.0, WORLD_SIZE.x - VIEWPORT_SIZE.x / 2.0),
        player
            .y
            .clamp(VIEWPORT_SIZE.y / 2.0, WORLD_SIZE.y - VIEWPORT_SIZE.y / 2.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_scene_is_valid_and_contains_every_required_content_kind() {
        let scene = build_scene(&QualificationGame::default());
        let prepared = Scene2DProcessor::prepare(&scene);
        assert!(
            prepared.diagnostics.is_empty(),
            "qualification scene diagnostics: {:?}",
            prepared.diagnostics
        );
        assert!(prepared
            .draws
            .iter()
            .any(|draw| matches!(draw, DrawCommand2D::Path { .. })));
        assert!(prepared
            .draws
            .iter()
            .any(|draw| matches!(draw, DrawCommand2D::Text { .. })));
        assert!(prepared
            .draws
            .iter()
            .any(|draw| matches!(draw, DrawCommand2D::Image { .. })));
        assert!(prepared
            .draws
            .iter()
            .any(|draw| matches!(draw, DrawCommand2D::Rectangle { .. })));
        assert!(scene
            .nodes
            .iter()
            .filter_map(|node| node.interaction.as_ref())
            .all(Interaction2D::is_interactive));
    }

    #[test]
    fn camera_tracks_the_player_inside_a_world_larger_than_the_viewport() {
        let mut game = QualificationGame::default();
        let initial = build_scene(&game).camera.center;
        game.player = Vec2::new(900.0, 360.0);
        let moved = build_scene(&game).camera.center;
        assert_ne!(initial, moved);
        assert!(WORLD_SIZE.x > VIEWPORT_SIZE.x);
        assert!(WORLD_SIZE.y > VIEWPORT_SIZE.y);
    }
}
