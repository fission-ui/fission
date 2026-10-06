use fission::game::{
    GameKey, GameRecorder, GameReplay, GameRuntime, HostInputEvent, PointerId, PointerKind,
    PointerPhase, SceneGesture, StepDuration,
};
use fission::scene3d::{AssetKind, RenderCapabilities3D, Scene3DProcessor, Vec2, Vec3};
use scene3d_qualification::{HarborGame, BEACON_NODE};

fn fixed_step() -> std::time::Duration {
    StepDuration::from_hz(60).as_duration()
}

fn key(key: GameKey, pressed: bool) -> HostInputEvent {
    HostInputEvent::Key { key, pressed }
}

fn drive_runtime(runtime: &mut GameRuntime<HarborGame>, key_code: GameKey, ticks: u32) {
    runtime.handle_input(key(key_code.clone(), true));
    for _ in 0..ticks {
        runtime.advance(fixed_step());
    }
    runtime.handle_input(key(key_code, false));
}

#[test]
fn packaged_static_gltf_supplies_a_textured_model() {
    let game = HarborGame::new();
    let mut runtime = GameRuntime::new(game);
    let scene = runtime.advance(std::time::Duration::ZERO).presentation;

    assert_eq!(scene.resources.models.len(), 1);
    assert_eq!(scene.resources.textures.len(), 1);
    assert!(scene
        .resources
        .materials
        .values()
        .any(|material| material.base_color_texture.is_some()));
    assert!(scene
        .assets
        .assets
        .iter()
        .any(|asset| asset.kind == AssetKind::Texture));
    assert!(scene.assets.validate().is_ok());
}

#[test]
fn viewport_coordinates_pick_the_stable_beacon_node() {
    let mut runtime = GameRuntime::new(HarborGame::new());
    let scene = runtime.advance(std::time::Duration::ZERO).presentation;
    let prepared = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
    assert!(prepared.is_renderable(), "{:?}", prepared.diagnostics);

    let hit = (0..54)
        .flat_map(|row| (0..96).map(move |column| (column, row)))
        .find_map(|(column, row)| {
            prepared.pick_viewport(Vec2::new(
                column as f32 * 10.0 + 5.0,
                row as f32 * 10.0 + 5.0,
            ))
        })
        .expect("the visible imported beacon is viewport-pickable");
    assert_eq!(hit.node, BEACON_NODE);
}

#[test]
fn kinematic_controller_collides_and_dynamic_cargo_falls() {
    let mut runtime = GameRuntime::new(HarborGame::new());
    let initial_cargo_y = runtime.state().cargo_position().y;
    drive_runtime(&mut runtime, GameKey::ArrowUp, 100);

    assert!(
        runtime.state().player_position().z > 1.45,
        "the central fixed cargo block must stop a straight run"
    );
    assert!(runtime.state().cargo_position().y < initial_cargo_y);
}

#[test]
fn one_semantic_move_is_a_usable_pointer_step() {
    let mut runtime = GameRuntime::new(HarborGame::new());
    let before = runtime.state().player_position();
    runtime.send(scene3d_qualification::HarborMessage::Move(
        scene3d_qualification::Direction::Right,
    ));
    runtime.advance(fixed_step());

    assert!(runtime.state().player_position().x - before.x > 0.4);
}

#[test]
fn pointer_and_semantic_selection_share_the_game_message_path() {
    let mut pointer_runtime = GameRuntime::new(HarborGame::new());
    pointer_runtime.handle_input(HostInputEvent::Pointer {
        id: PointerId(1),
        kind: PointerKind::Touch,
        phase: PointerPhase::Down,
        button: None,
        position: Vec2::new(480.0, 270.0),
        target: Some(BEACON_NODE),
    });
    pointer_runtime.handle_input(HostInputEvent::SceneGesture {
        node: BEACON_NODE,
        gesture: SceneGesture::Activate,
    });
    pointer_runtime.advance(fixed_step());

    let mut keyboard_runtime = GameRuntime::new(HarborGame::new());
    keyboard_runtime.handle_input(key(GameKey::Confirm, true));
    keyboard_runtime.advance(fixed_step());

    assert!(pointer_runtime.state().beacon_selected());
    assert!(keyboard_runtime.state().beacon_selected());
}

#[test]
fn complete_run_restores_from_snapshot_and_replays_identically() {
    let mut recorder = GameRecorder::new(HarborGame::new());
    recorder.handle_input(HostInputEvent::SceneGesture {
        node: BEACON_NODE,
        gesture: SceneGesture::Activate,
    });
    let mut expected_last = recorder.advance(fixed_step());

    recorder.handle_input(key(GameKey::ArrowRight, true));
    for _ in 0..30 {
        expected_last = recorder.advance(fixed_step());
    }
    recorder.handle_input(key(GameKey::ArrowRight, false));
    recorder.handle_input(key(GameKey::ArrowUp, true));
    for _ in 0..142 {
        expected_last = recorder.advance(fixed_step());
    }
    recorder.handle_input(key(GameKey::ArrowUp, false));
    recorder.handle_input(key(GameKey::ArrowLeft, true));
    for _ in 0..22 {
        expected_last = recorder.advance(fixed_step());
    }
    recorder.handle_input(key(GameKey::ArrowLeft, false));

    assert!(recorder.state().success());
    let expected_player = recorder.state().player_position();
    let replay: GameReplay<HarborGame, _> = recorder.finish();
    let replayed = GameRuntime::<HarborGame>::replay(replay).expect("recorded run replays");
    assert!(replayed.runtime.state().success());
    assert_eq!(replayed.runtime.state().player_position(), expected_player);
    assert_eq!(replayed.frames.last(), Some(&expected_last));

    let mut original = GameRuntime::new(HarborGame::new());
    original.handle_input(HostInputEvent::SceneGesture {
        node: BEACON_NODE,
        gesture: SceneGesture::Activate,
    });
    original.advance(fixed_step());
    drive_runtime(&mut original, GameKey::ArrowRight, 30);
    // Capture before the dynamic ball reaches the floor. The alpha provider's
    // portable snapshot intentionally stores bodies/contacts rather than
    // backend-specific solver warm-start caches.
    drive_runtime(&mut original, GameKey::ArrowUp, 5);
    let mut restored = GameRuntime::from_snapshot(original.snapshot()).expect("snapshot restores");

    drive_runtime(&mut original, GameKey::ArrowUp, 137);
    drive_runtime(&mut restored, GameKey::ArrowUp, 137);
    drive_runtime(&mut original, GameKey::ArrowLeft, 22);
    drive_runtime(&mut restored, GameKey::ArrowLeft, 22);
    let expected = original.advance(std::time::Duration::ZERO);
    let actual = restored.advance(std::time::Duration::ZERO);

    assert!(original.state().success());
    assert!(restored.state().success());
    assert_eq!(actual, expected);
    assert_eq!(
        restored.state().physics_snapshot(),
        original.state().physics_snapshot()
    );
}

#[test]
fn following_camera_tracks_the_player_in_world_space() {
    let mut runtime = GameRuntime::new(HarborGame::new());
    let before = runtime
        .advance(std::time::Duration::ZERO)
        .presentation
        .camera;
    drive_runtime(&mut runtime, GameKey::ArrowRight, 12);
    let after = runtime
        .advance(std::time::Duration::ZERO)
        .presentation
        .camera;

    assert!(after.eye.x > before.eye.x);
    assert!(after.target.x > before.target.x);
    assert_eq!(after.eye.y, before.eye.y);
    assert!(Vec3::new(after.eye.x, after.eye.y, after.eye.z).is_finite());
}
