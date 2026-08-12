use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

use fission_render::backend::{
    GraphicsBackendDriver, GraphicsBackendSession, ReadbackRequest, SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, DisplayOpKind, RenderMode};
use fission_render::external_surface::ExternalSurfaceBindings;
use fission_render::frame::{
    DamageRegion, FrameId, FrameMetadata, FrameViewport, InteractiveFrame, ResourceEpoch,
    SemanticsEpoch,
};
use fission_render::resource::{
    ResourceContentIdentity, ResourceEntry, ResourceId, ResourceKind, ResourcePayload,
    ResourceProvenance, ResourceSnapshot, ResourceSource,
};
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, ScaleFactor, SessionState, SurfaceDescriptor,
    SurfaceId, SurfaceKind, SurfaceTarget, ThreadAffinity,
};
use fission_render::{Color, DisplayList, DisplayOp, Fill, LayoutRect, LayoutSize, RenderScene};
use fission_skia_sys::web::{
    decode, decode_commands, encode, Ack, DestroyReason, ErrorCode, ErrorPacket, Message, Packet,
    ProtocolSession, ResourceOperation, SessionId, DEFAULT_DECODE_LIMITS,
};

use super::driver::{damage_rects, CanvasKitBackendPreference, CanvasKitDriver};
use super::host::CanvasKitHost;
use super::resources::ResourceMap;

#[derive(Debug)]
struct TestTarget {
    descriptor: SurfaceDescriptor,
}

impl TestTarget {
    fn new(size: PhysicalSize, scale_factor: f64) -> Self {
        Self {
            descriptor: SurfaceDescriptor {
                id: SurfaceId(7),
                kind: SurfaceKind::WebCanvas,
                size,
                scale_factor: ScaleFactor::new(scale_factor).unwrap(),
                color_format: ColorFormat::Rgba8Srgb,
                thread_affinity: ThreadAffinity::MainThread,
            },
        }
    }
}

impl SurfaceTarget for TestTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
struct MockHostError(&'static str);

impl fmt::Display for MockHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone)]
enum Reply {
    Ack,
    Error(ErrorCode, &'static str),
    WrongAck,
    Malformed,
    TransportError,
}

#[derive(Default)]
struct MockState {
    protocol: ProtocolSession,
    requests: Vec<Message>,
    replies: VecDeque<Reply>,
    response_session: Option<SessionId>,
    response_sequence: u64,
    lifecycle_events: VecDeque<Vec<u8>>,
}

#[derive(Clone, Default)]
struct MockControl(Rc<RefCell<MockState>>);

impl MockControl {
    fn host(&self) -> MockHost {
        MockHost(self.0.clone())
    }

    fn reply(&self, reply: Reply) {
        self.0.borrow_mut().replies.push_back(reply);
    }

    fn requests(&self) -> Vec<Message> {
        self.0.borrow().requests.clone()
    }

    fn context_lost(&self) {
        let failed_sequence = self
            .0
            .borrow()
            .requests
            .last()
            .expect("context loss follows an acknowledged command")
            .envelope
            .sequence;
        self.lifecycle_packet(Packet::Error(ErrorPacket {
            failed_sequence,
            code: ErrorCode::SurfaceLost,
            message: "injected browser context loss".into(),
        }));
    }

    fn context_restored(&self) {
        let acknowledged_sequence = self
            .0
            .borrow()
            .requests
            .last()
            .expect("context restoration follows an acknowledged command")
            .envelope
            .sequence;
        self.lifecycle_packet(Packet::Ack(Ack {
            acknowledged_sequence,
        }));
    }

    fn lifecycle_packet(&self, packet: Packet) {
        let mut state = self.0.borrow_mut();
        let session = state
            .response_session
            .expect("an acknowledged Init establishes the response session");
        state.response_sequence = state.response_sequence.saturating_add(1);
        let sequence = state.response_sequence;
        let packet = encode(&Message::new(session, sequence, packet))
            .expect("the mock lifecycle packet is canonical");
        state.lifecycle_events.push_back(packet);
    }
}

struct MockHost(Rc<RefCell<MockState>>);

impl CanvasKitHost for MockHost {
    type Error = MockHostError;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        let message = decode(&request, &DEFAULT_DECODE_LIMITS)
            .expect("the driver must send canonical request packets");
        let mut state = self.0.borrow_mut();
        state.requests.push(message.clone());
        let reply = state.replies.pop_front().unwrap_or(Reply::Ack);
        if matches!(&reply, Reply::TransportError) {
            return Err(MockHostError("injected transport failure"));
        }
        if matches!(&reply, Reply::Malformed) {
            return Ok(vec![0, 1, 2]);
        }

        if state.response_session != Some(message.envelope.session) {
            state.response_session = Some(message.envelope.session);
            state.response_sequence = 0;
        }
        state.response_sequence += 1;

        let wrong_ack = matches!(&reply, Reply::WrongAck);
        let response_packet = match reply {
            Reply::Ack | Reply::WrongAck => {
                let mut candidate = state.protocol.clone();
                candidate
                    .accept(&message)
                    .expect("the driver request sequence must satisfy the host protocol");
                state.protocol = candidate;
                Packet::Ack(Ack {
                    acknowledged_sequence: if wrong_ack {
                        message.envelope.sequence.saturating_add(1)
                    } else {
                        message.envelope.sequence
                    },
                })
            }
            Reply::Error(code, text) => Packet::Error(ErrorPacket {
                failed_sequence: message.envelope.sequence,
                code,
                message: text.into(),
            }),
            Reply::Malformed | Reply::TransportError => unreachable!(),
        };
        encode(&Message::new(
            message.envelope.session,
            state.response_sequence,
            response_packet,
        ))
        .map_err(|_| MockHostError("response encoding failed"))
    }

    fn poll_lifecycle_event(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.0.borrow_mut().lifecycle_events.pop_front())
    }
}

struct FrameFixture {
    scene: RenderScene,
    metadata: FrameMetadata,
    resources: ResourceSnapshot,
    bindings: ExternalSurfaceBindings,
    clear_color: Color,
}

impl FrameFixture {
    fn new(
        frame_id: u64,
        resource_epoch: u64,
        size: PhysicalSize,
        scale_factor: f64,
        damage: DamageRegion,
        resources: ResourceSnapshot,
    ) -> Self {
        let logical_size = LayoutSize {
            width: size.width as f32 / scale_factor as f32,
            height: size.height as f32 / scale_factor as f32,
        };
        let bounds = LayoutRect::new(0.0, 0.0, logical_size.width, logical_size.height);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawRect {
            rect: LayoutRect::new(1.0, 1.0, 8.0, 6.0),
            fill: Some(Fill::Solid(Color {
                r: 20,
                g: 40,
                b: 60,
                a: 255,
            })),
            stroke: None,
            corner_radius: 2.0,
            shadow: None,
            bounds: LayoutRect::new(1.0, 1.0, 8.0, 6.0),
            node_id: None,
        });
        Self {
            scene: RenderScene::from_display_list(list),
            metadata: FrameMetadata {
                frame_id: FrameId(frame_id),
                viewport: FrameViewport {
                    logical_size,
                    physical_size: size,
                    scale_factor: ScaleFactor::new(scale_factor).unwrap(),
                },
                damage,
                resource_epoch: ResourceEpoch(resource_epoch),
                semantics_epoch: SemanticsEpoch(frame_id),
            },
            resources,
            bindings: ExternalSurfaceBindings::new(),
            clear_color: Color {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
        }
    }

    fn frame(&self) -> InteractiveFrame<'_> {
        InteractiveFrame::new(&self.scene, &self.metadata, &self.resources, &self.bindings)
            .with_clear_color(self.clear_color)
    }
}

fn ready_resource(id: u64, identity: &str, bytes: &[u8]) -> ResourceEntry {
    ResourceEntry::ready(
        ResourceId(id),
        ResourceContentIdentity::try_new(identity).unwrap(),
        ResourceKind::Image,
        ResourceProvenance::new(ResourceSource::Memory),
        ResourcePayload::Bytes(bytes.to_vec()),
    )
}

#[test]
fn strict_lifecycle_sends_init_batches_frames_resize_and_destroy() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Auto);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(100, 50), 2.0);
    session.attach(&target).unwrap();

    let resources = ResourceSnapshot::empty(ResourceEpoch(1));
    let fixture = FrameFixture::new(
        4,
        1,
        target.descriptor.size,
        2.0,
        DamageRegion::Rects(vec![LayoutRect::new(-1.2, 1.25, 4.0, 3.0)]),
        resources,
    );
    let report = session.render(&fixture.frame()).unwrap();
    assert_eq!(report.frame_id, Some(FrameId(4)));
    assert_eq!(report.encoded_operations, 1);
    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(4)));

    session
        .resize(SurfaceMetrics {
            size: PhysicalSize::new(120, 60),
            scale_factor: ScaleFactor::new(2.0).unwrap(),
        })
        .unwrap();
    session.detach().unwrap();

    let requests = control.requests();
    assert_eq!(requests.len(), 5);
    assert!(matches!(requests[0].packet, Packet::Init(_)));
    assert!(matches!(requests[1].packet, Packet::ResourceBatch(_)));
    let Packet::Frame(frame) = &requests[2].packet else {
        panic!("expected a Frame packet");
    };
    assert_eq!(frame.frame_id, 4);
    assert_eq!(
        frame.damage,
        vec![fission_skia_sys::web::DamageRect {
            x: 0,
            y: 2,
            width: 6,
            height: 7,
        }]
    );
    assert_eq!(
        frame.clear_color,
        [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0]
    );
    assert!(!decode_commands(&frame.commands).unwrap().is_empty());
    assert!(matches!(requests[3].packet, Packet::Resize(_)));
    assert!(matches!(requests[4].packet, Packet::Destroy(_)));
    assert_eq!(
        requests
            .iter()
            .map(|request| request.envelope.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn resource_error_is_retryable_without_advancing_sequence_or_generation() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Software);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(64, 32), 1.0);
    session.attach(&target).unwrap();

    control.reply(Reply::Error(
        ErrorCode::ResourceFailure,
        "injected decode failure",
    ));
    let resources = ResourceSnapshot::try_new(
        ResourceEpoch(3),
        [ready_resource(9, "image-v1", &[1, 2, 3])],
    )
    .unwrap();
    let fixture = FrameFixture::new(
        1,
        3,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        resources,
    );
    let error = session.render(&fixture.frame()).unwrap_err();
    assert_eq!(error.code, "canvaskit-host-resource-failure");

    let report = session.render(&fixture.frame()).unwrap();
    assert_eq!(report.uploaded_bytes, 3);
    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(1)));

    let requests = control.requests();
    let batches = requests
        .iter()
        .filter_map(|request| match &request.packet {
            Packet::ResourceBatch(batch) => Some((request.envelope.sequence, batch)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].0, 2);
    assert_eq!(batches[1].0, 2);
    assert_eq!(batches[0].1, batches[1].1);
    assert_eq!(batches[1].1.updates[0].handle.generation, 1);
}

#[test]
fn present_waits_for_the_frame_ack_and_same_frame_can_retry() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::WebGl);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(80, 40), 1.0);
    session.attach(&target).unwrap();

    control.reply(Reply::Ack);
    control.reply(Reply::Error(ErrorCode::InvalidState, "frame rejected"));
    let fixture = FrameFixture::new(
        7,
        1,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    assert_eq!(
        session.render(&fixture.frame()).unwrap_err().code,
        "canvaskit-host-invalid-state"
    );
    assert_eq!(
        session.present().unwrap_err().code,
        "canvaskit-present-without-pending-frame"
    );

    session.render(&fixture.frame()).unwrap();
    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(7)));
    assert_eq!(
        session.present().unwrap_err().code,
        "canvaskit-present-without-pending-frame"
    );
    let frame_sequences = control
        .requests()
        .into_iter()
        .filter_map(|request| {
            matches!(request.packet, Packet::Frame(_)).then_some(request.envelope.sequence)
        })
        .collect::<Vec<_>>();
    assert_eq!(frame_sequences, vec![3, 3]);
}

#[test]
fn one_acknowledged_frame_owns_the_single_pending_present_slot() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Software);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(80, 40), 1.0);
    session.attach(&target).unwrap();

    let first = FrameFixture::new(
        1,
        1,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    let second = FrameFixture::new(
        2,
        1,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    session.render(&first.frame()).unwrap();
    let request_count = control.requests().len();

    assert_eq!(
        session.render(&second.frame()).unwrap_err().code,
        "canvaskit-present-pending"
    );
    assert_eq!(
        session
            .resize(SurfaceMetrics {
                size: target.descriptor.size,
                scale_factor: target.descriptor.scale_factor,
            })
            .unwrap_err()
            .code,
        "canvaskit-present-pending"
    );
    assert_eq!(control.requests().len(), request_count);

    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(1)));
    assert_eq!(
        session.present().unwrap_err().code,
        "canvaskit-present-without-pending-frame"
    );
    session.render(&second.frame()).unwrap();
    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(2)));
}

#[test]
fn resize_does_not_erase_the_session_frame_monotonicity_gate() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Auto);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(40, 20), 1.0);
    session.attach(&target).unwrap();

    let first = FrameFixture::new(
        4,
        1,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    session.render(&first.frame()).unwrap();
    session.present().unwrap();

    let resized = PhysicalSize::new(60, 30);
    session
        .resize(SurfaceMetrics {
            size: resized,
            scale_factor: ScaleFactor::ONE,
        })
        .unwrap();
    let stale = FrameFixture::new(
        4,
        1,
        resized,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    assert_eq!(
        session.render(&stale.frame()).unwrap_err().code,
        "canvaskit-frame-id-not-monotonic"
    );

    let next = FrameFixture::new(
        5,
        1,
        resized,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    session.render(&next.frame()).unwrap();
    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(5)));
}

#[test]
fn queued_context_loss_and_restoration_reconcile_response_sequence() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::WebGl);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(64, 32), 1.0);
    session.attach(&target).unwrap();

    control.context_lost();
    control.context_restored();
    assert_eq!(
        session.recover(LossKind::Surface).unwrap(),
        Recovery::Reattached
    );
    assert_eq!(session.state(), SessionState::Attached);
    assert_eq!(control.requests().len(), 1);

    let fixture = FrameFixture::new(
        1,
        1,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        ResourceSnapshot::empty(ResourceEpoch(1)),
    );
    session.render(&fixture.frame()).unwrap();
    session.present().unwrap();
    assert_eq!(session.diagnostics().counters.surface_recoveries, 1);
}

#[test]
fn failed_initial_surface_can_reinitialize_from_retained_target_metrics() {
    let control = MockControl::default();
    control.reply(Reply::Error(
        ErrorCode::SurfaceLost,
        "injected initial surface failure",
    ));
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::WebGl);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(64, 32), 1.0);

    assert_eq!(
        session.attach(&target).unwrap_err().code,
        "canvaskit-host-surface-lost"
    );
    assert_eq!(session.state(), SessionState::Lost);
    assert_eq!(
        session.recover(LossKind::Surface).unwrap(),
        Recovery::Reattached
    );
    assert_eq!(session.state(), SessionState::Attached);
    assert_eq!(
        control
            .requests()
            .iter()
            .filter_map(|request| {
                matches!(request.packet, Packet::Init(_)).then_some(request.envelope.session.get())
            })
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn unresolved_context_loss_reinitializes_and_reuploads_the_session() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::WebGl);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(64, 32), 1.0);
    session.attach(&target).unwrap();

    let resources = ResourceSnapshot::try_new(
        ResourceEpoch(2),
        [ready_resource(9, "image-v1", &[1, 2, 3])],
    )
    .unwrap();
    let first = FrameFixture::new(
        1,
        2,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        resources.clone(),
    );
    session.render(&first.frame()).unwrap();
    session.present().unwrap();

    control.context_lost();
    assert_eq!(
        session.recover(LossKind::Surface).unwrap(),
        Recovery::Reattached
    );
    let second = FrameFixture::new(
        2,
        2,
        target.descriptor.size,
        1.0,
        DamageRegion::Full,
        resources,
    );
    assert_eq!(session.render(&second.frame()).unwrap().uploaded_bytes, 3);
    session.present().unwrap();

    let requests = control.requests();
    let destroy = requests
        .iter()
        .find_map(|request| match &request.packet {
            Packet::Destroy(destroy) => Some((request.envelope.session, destroy.reason)),
            _ => None,
        })
        .expect("surface recovery retires the lost session");
    assert_eq!(destroy.1, DestroyReason::ContextLost);
    let init_sessions = requests
        .iter()
        .filter_map(|request| {
            matches!(request.packet, Packet::Init(_)).then_some(request.envelope.session.get())
        })
        .collect::<Vec<_>>();
    assert_eq!(init_sessions, vec![1, 2]);
    assert_eq!(
        requests
            .iter()
            .filter(|request| matches!(request.packet, Packet::ResourceBatch(_)))
            .count(),
        2
    );
}

#[test]
fn device_recreation_is_not_claimed_without_a_host_factory() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Auto);
    assert!(driver.capabilities().surface_loss_recovery);
    assert!(!driver.capabilities().device_loss_recovery);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(32, 16), 1.0);
    session.attach(&target).unwrap();

    assert_eq!(
        session.recover(LossKind::Device).unwrap(),
        Recovery::Unrecoverable
    );
    assert_eq!(session.state(), SessionState::Lost);
    assert_eq!(control.requests().len(), 1);
    session.detach().unwrap();
}

#[test]
fn suspend_and_resume_start_a_strictly_new_session() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Auto);
    let mut session = GraphicsBackendSession::new(driver).unwrap();
    let target = TestTarget::new(PhysicalSize::new(32, 16), 1.0);
    session.attach(&target).unwrap();
    session.suspend().unwrap();
    session.resume(&target).unwrap();
    session.detach().unwrap();

    let requests = control.requests();
    assert_eq!(requests.len(), 4);
    assert!(matches!(requests[0].packet, Packet::Init(_)));
    assert!(matches!(requests[1].packet, Packet::Destroy(_)));
    assert!(matches!(requests[2].packet, Packet::Init(_)));
    assert!(matches!(requests[3].packet, Packet::Destroy(_)));
    assert_eq!(requests[0].envelope.session.get(), 1);
    assert_eq!(requests[1].envelope.session.get(), 1);
    assert_eq!(requests[2].envelope.session.get(), 2);
    assert_eq!(requests[3].envelope.session.get(), 2);
    assert_eq!(requests[0].envelope.sequence, 1);
    assert_eq!(requests[1].envelope.sequence, 2);
    assert_eq!(requests[2].envelope.sequence, 1);
    assert_eq!(requests[3].envelope.sequence, 2);
}

#[test]
fn dropping_an_attached_driver_retires_the_host_session() {
    let control = MockControl::default();
    {
        let mut driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Software);
        let target = TestTarget::new(PhysicalSize::new(32, 16), 1.0);
        driver.attach(&target).unwrap();
    }

    let requests = control.requests();
    assert_eq!(requests.len(), 2);
    assert!(matches!(requests[0].packet, Packet::Init(_)));
    assert!(matches!(
        &requests[1].packet,
        Packet::Destroy(fission_skia_sys::web::Destroy {
            reason: DestroyReason::HostShutdown
        })
    ));
}

#[test]
fn malformed_or_mismatched_responses_poison_the_session() {
    for reply in [Reply::WrongAck, Reply::Malformed, Reply::TransportError] {
        let control = MockControl::default();
        control.reply(reply);
        let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Auto);
        let mut session = GraphicsBackendSession::new(driver).unwrap();
        let target = TestTarget::new(PhysicalSize::new(32, 16), 1.0);
        assert!(session.attach(&target).is_err());
        assert_eq!(session.state(), SessionState::Lost);
    }
}

#[test]
fn unsupported_readback_and_memory_pressure_fail_explicitly() {
    let control = MockControl::default();
    let mut driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Auto);
    let target = TestTarget::new(PhysicalSize::new(32, 16), 1.0);
    driver.attach(&target).unwrap();

    let readback = driver
        .readback(ReadbackRequest {
            region: None,
            color_format: ColorFormat::Rgba8Srgb,
        })
        .unwrap_err();
    assert_eq!(readback.code, "canvaskit-readback-unsupported");
    let pressure = driver.trim_memory(MemoryPressure::Critical).unwrap_err();
    assert_eq!(pressure.code, "canvaskit-memory-pressure-unsupported");
    assert!(!driver.capabilities().readback);
    assert!(!driver
        .capabilities()
        .supports_display_op(DisplayOpKind::DrawImage));
    assert!(!driver
        .capabilities()
        .supports_display_op(DisplayOpKind::CachedScene));
    assert!(driver.capabilities().surface_loss_recovery);
    assert!(!driver.capabilities().device_loss_recovery);
}

#[test]
fn graphite_is_reported_unavailable_until_the_executor_implements_it() {
    let control = MockControl::default();
    let driver = CanvasKitDriver::new(control.host(), CanvasKitBackendPreference::Graphite);

    assert!(driver.capabilities().render_modes.is_empty());
    assert!(!driver
        .capabilities()
        .render_modes
        .contains(&RenderMode::Gpu));
    assert!(driver.capabilities().display_ops.is_empty());
    assert!(!driver.capabilities().surface_loss_recovery);
    assert!(!driver.capabilities().device_loss_recovery);
}

#[test]
fn resource_plans_are_atomic_deterministic_and_generational() {
    let mut resources = ResourceMap::default();
    let first_snapshot = ResourceSnapshot::try_new(
        ResourceEpoch(1),
        [
            ready_resource(8, "same-content", &[9, 8, 7]),
            ready_resource(2, "same-content", &[9, 8, 7]),
        ],
    )
    .unwrap();
    let first = resources.plan(&first_snapshot).unwrap().unwrap();
    let retried = resources.plan(&first_snapshot).unwrap().unwrap();
    assert_eq!(first.batch, retried.batch);
    assert_eq!(first.batch.updates.len(), 2);
    assert_eq!(first.batch.updates[0].handle.slot, 1);
    assert_eq!(first.batch.updates[1].handle.slot, 2);
    assert_eq!(
        first.batch.updates[0].content_id,
        first.batch.updates[1].content_id
    );
    assert_ne!(first.batch.updates[0].content_id, 0);
    resources.commit(first);
    assert_eq!(resources.epoch(), 1);

    let replacement = ResourceSnapshot::try_new(
        ResourceEpoch(2),
        [ready_resource(2, "new-content", &[1, 2, 3])],
    )
    .unwrap();
    let second = resources.plan(&replacement).unwrap().unwrap();
    assert_eq!(second.batch.updates.len(), 3);
    assert_eq!(
        second.batch.updates[0].operation,
        ResourceOperation::Release
    );
    assert_eq!(
        second.batch.updates[1].operation,
        ResourceOperation::Release
    );
    let replacement_update = second
        .batch
        .updates
        .iter()
        .find(|update| update.operation == ResourceOperation::Upsert)
        .unwrap();
    assert_eq!(replacement_update.handle.slot, 1);
    assert_eq!(replacement_update.handle.generation, 2);
    let replacement_handle = replacement_update.handle;
    resources.commit(second);
    assert_eq!(resources.epoch(), 2);
    assert_eq!(resources.handle(ResourceId(2)), Some(replacement_handle));

    let empty = ResourceSnapshot::empty(ResourceEpoch(3));
    let third = resources.plan(&empty).unwrap().unwrap();
    resources.commit(third);
    let new_resource =
        ResourceSnapshot::try_new(ResourceEpoch(4), [ready_resource(99, "other", &[4])]).unwrap();
    let fourth = resources.plan(&new_resource).unwrap().unwrap();
    let upsert = fourth
        .batch
        .updates
        .iter()
        .find(|update| update.operation == ResourceOperation::Upsert)
        .unwrap();
    assert_eq!(upsert.handle.slot, 1);
    assert_eq!(upsert.handle.generation, 3);
}

#[test]
fn damage_conversion_rounds_outward_and_clips_to_the_surface() {
    let metrics = SurfaceMetrics {
        size: PhysicalSize::new(20, 10),
        scale_factor: ScaleFactor::new(2.0).unwrap(),
    };
    assert!(damage_rects(&DamageRegion::None, metrics).is_empty());
    assert_eq!(
        damage_rects(&DamageRegion::Full, metrics),
        vec![fission_skia_sys::web::DamageRect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        }]
    );
    assert_eq!(
        damage_rects(
            &DamageRegion::Rects(vec![
                LayoutRect::new(-2.0, -1.0, 3.25, 2.25),
                LayoutRect::new(1.25, 1.25, 0.0, 2.0),
                LayoutRect::new(20.0, 20.0, 1.0, 1.0),
            ]),
            metrics,
        ),
        vec![fission_skia_sys::web::DamageRect {
            x: 0,
            y: 0,
            width: 3,
            height: 3,
        }]
    );
}
