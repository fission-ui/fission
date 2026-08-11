use super::*;

const SESSION_VALUE: u64 = 0x0102_0304_0506_0708;

fn session() -> SessionId {
    SessionId::new(SESSION_VALUE).unwrap()
}

fn surface() -> SurfaceSize {
    SurfaceSize {
        width: 1_280,
        height: 720,
        scale_factor: 1.5,
    }
}

fn init_message() -> Message {
    Message::new(
        session(),
        1,
        Packet::Init(Init {
            surface: surface(),
            backend: BackendPreference::Auto,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
        }),
    )
}

fn resource_message(sequence: u64, epoch: u64, update: ResourceUpdate) -> Message {
    Message::new(
        session(),
        sequence,
        Packet::ResourceBatch(ResourceBatch {
            resource_epoch: epoch,
            updates: vec![update],
        }),
    )
}

fn upsert(generation: u32) -> ResourceUpdate {
    ResourceUpdate {
        handle: ResourceHandle {
            slot: 3,
            generation,
        },
        operation: ResourceOperation::Upsert,
        kind: ResourceKind::Image,
        content_id: 0x1122_3344_5566_7788,
        bytes: vec![0xaa, 0xbb, 0xcc],
    }
}

fn release(generation: u32) -> ResourceUpdate {
    ResourceUpdate {
        handle: ResourceHandle {
            slot: 3,
            generation,
        },
        operation: ResourceOperation::Release,
        kind: ResourceKind::Image,
        content_id: 0,
        bytes: Vec::new(),
    }
}

fn frame_message(sequence: u64, frame_id: u64, resource_epoch: u64) -> Message {
    Message::new(
        session(),
        sequence,
        Packet::Frame(Frame {
            frame_id,
            resource_epoch,
            semantics_epoch: 5,
            surface: surface(),
            clear_color: [0.25, 0.5, 0.75, 1.0],
            damage: vec![DamageRect {
                x: 10,
                y: 20,
                width: 100,
                height: 50,
            }],
            commands: vec![0xde, 0xad, 0xbe, 0xef],
        }),
    )
}

#[test]
fn init_has_stable_golden_encoding() {
    let expected = [
        0x46, 0x53, 0x4b, 0x4e, 0x01, 0x00, 0x01, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0xd0, 0x02, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x3f, 0x00,
        0x01, 0x02, 0x00,
    ];
    let encoded = encode(&init_message()).unwrap();
    assert_eq!(encoded, expected);
    assert_eq!(
        decode(&expected, &DEFAULT_DECODE_LIMITS).unwrap(),
        init_message()
    );
}

#[test]
fn resource_batch_has_stable_golden_encoding() {
    let expected = [
        0x46, 0x53, 0x4b, 0x4e, 0x01, 0x00, 0x03, 0x00, 0x4b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x03, 0x00, 0x00, 0x00, 0xaa, 0xbb, 0xcc,
    ];
    let message = resource_message(2, 7, upsert(2));
    let encoded = encode(&message).unwrap();
    assert_eq!(encoded, expected);
    assert_eq!(decode(&expected, &DEFAULT_DECODE_LIMITS).unwrap(), message);
}

#[test]
fn frame_has_stable_golden_encoding() {
    let expected = [
        0x46, 0x53, 0x4b, 0x4e, 0x01, 0x00, 0x04, 0x00, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00,
        0xd0, 0x02, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x3f, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x80, 0x3e, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x40, 0x3f, 0x00, 0x00,
        0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x64,
        0x00, 0x00, 0x00, 0x32, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef,
    ];
    let message = frame_message(3, 9, 7);
    let encoded = encode(&message).unwrap();
    assert_eq!(encoded, expected);
    assert_eq!(decode(&expected, &DEFAULT_DECODE_LIMITS).unwrap(), message);
}

#[test]
fn every_packet_kind_round_trips() {
    let messages = [
        init_message(),
        Message::new(
            session(),
            2,
            Packet::Resize(Resize {
                surface: SurfaceSize {
                    width: 640,
                    height: 480,
                    scale_factor: 2.0,
                },
            }),
        ),
        resource_message(3, 1, upsert(1)),
        frame_message(4, 1, 0),
        Message::new(
            session(),
            5,
            Packet::Destroy(Destroy {
                reason: DestroyReason::HostShutdown,
            }),
        ),
        Message::new(
            session(),
            6,
            Packet::Ack(Ack {
                acknowledged_sequence: 5,
            }),
        ),
        Message::new(
            session(),
            7,
            Packet::Error(ErrorPacket {
                failed_sequence: 6,
                code: ErrorCode::SurfaceLost,
                message: "surface lost".into(),
            }),
        ),
    ];

    for message in messages {
        let encoded = encode(&message).unwrap();
        assert_eq!(decode(&encoded, &DEFAULT_DECODE_LIMITS).unwrap(), message);
    }
}

#[test]
fn malformed_lengths_reserved_fields_and_limits_are_rejected() {
    let encoded = encode(&init_message()).unwrap();

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(matches!(
        decode(&trailing, &DEFAULT_DECODE_LIMITS),
        Err(ProtocolError::LengthMismatch { .. })
    ));

    let mut reserved = encoded.clone();
    reserved[12] = 1;
    assert_eq!(
        decode(&reserved, &DEFAULT_DECODE_LIMITS),
        Err(ProtocolError::NonZeroFlags(1))
    );

    let limits = DecodeLimits {
        max_surface_dimension: 1_000,
        ..DEFAULT_DECODE_LIMITS
    };
    assert!(matches!(
        decode(&encoded, &limits),
        Err(ProtocolError::LimitExceeded {
            field: "surface dimension",
            ..
        })
    ));
}

#[test]
fn session_rejects_stale_sequences_sessions_and_resource_generations_atomically() {
    let mut state = ProtocolSession::default();
    state.accept(&init_message()).unwrap();
    state.accept(&resource_message(2, 7, upsert(2))).unwrap();
    state.accept(&frame_message(3, 9, 7)).unwrap();
    assert_eq!(state.live_resource_count(), 1);

    assert!(matches!(
        state.accept(&frame_message(3, 10, 7)),
        Err(ProtocolError::StaleSequence { .. })
    ));

    let stale_generation = resource_message(4, 8, release(1));
    assert!(matches!(
        state.accept(&stale_generation),
        Err(ProtocolError::StaleResourceGeneration { .. })
    ));

    state.accept(&resource_message(4, 8, release(2))).unwrap();
    assert_eq!(state.live_resource_count(), 0);
    assert!(matches!(
        state.accept(&resource_message(5, 9, upsert(2))),
        Err(ProtocolError::ReleasedResourceGeneration(_))
    ));
    state.accept(&resource_message(5, 9, upsert(3))).unwrap();

    state
        .accept(&Message::new(
            session(),
            6,
            Packet::Destroy(Destroy {
                reason: DestroyReason::Normal,
            }),
        ))
        .unwrap();
    assert_eq!(state.active_session(), None);
    assert!(matches!(
        state.accept(&init_message()),
        Err(ProtocolError::StaleSession { .. })
    ));

    let next_session = SessionId::new(SESSION_VALUE + 1).unwrap();
    let next_init = Message::new(
        next_session,
        1,
        Packet::Init(Init {
            surface: surface(),
            backend: BackendPreference::Software,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Opaque,
        }),
    );
    state.accept(&next_init).unwrap();
    assert_eq!(state.active_session(), Some(next_session));
}
