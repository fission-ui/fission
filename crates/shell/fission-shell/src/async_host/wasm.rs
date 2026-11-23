use fission_core::{
    ActionEnvelope, CapabilityCtx, CapabilityType, DataStreamRegistry, InlineJob,
    JobCancellationToken, JobCtx, JobRef, JobSpec, OperationCapability, ResourceExecutionContext,
    ServiceCtx, ServiceRunner, ServiceSpec, ServiceType,
};
use futures_channel::mpsc as async_mpsc;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{mpsc, Arc, Mutex};

pub type WakeFn = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug)]
pub enum AsyncMessage {
    JobOk {
        job_name: String,
        req_id: u64,
        payload: Vec<u8>,
        on_ok: Option<ActionEnvelope>,
        resource: Option<ResourceExecutionContext>,
    },
    JobErr {
        job_name: String,
        req_id: u64,
        payload: Option<Vec<u8>>,
        on_err: Option<ActionEnvelope>,
        message: Option<String>,
        resource: Option<ResourceExecutionContext>,
    },
    ServiceStarted {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        resource: Option<ResourceExecutionContext>,
    },
    ServiceStartFailed {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        payload: Option<Vec<u8>>,
        message: Option<String>,
        resource: Option<ResourceExecutionContext>,
    },
    ServiceEvent {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        payload: Vec<u8>,
        resource: Option<ResourceExecutionContext>,
    },
    ServiceStopped {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        resource: Option<ResourceExecutionContext>,
    },
    ServiceCommandOk {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        req_id: u64,
        payload: Option<Vec<u8>>,
        on_ok: Option<ActionEnvelope>,
        resource: Option<ResourceExecutionContext>,
    },
    ServiceCommandErr {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        req_id: u64,
        payload: Option<Vec<u8>>,
        on_err: Option<ActionEnvelope>,
        message: Option<String>,
        resource: Option<ResourceExecutionContext>,
    },
    CapabilityOk {
        capability_name: String,
        req_id: u64,
        payload: Vec<u8>,
        on_ok: Option<ActionEnvelope>,
        resource: Option<ResourceExecutionContext>,
    },
    CapabilityErr {
        capability_name: String,
        req_id: u64,
        payload: Option<Vec<u8>>,
        on_err: Option<ActionEnvelope>,
        message: Option<String>,
        resource: Option<ResourceExecutionContext>,
    },
}

#[derive(Clone)]
pub enum ServiceControlMessage {
    Command {
        req_id: u64,
        payload: Vec<u8>,
        on_ok: Option<ActionEnvelope>,
        on_err: Option<ActionEnvelope>,
    },
    Stop,
}

#[derive(Clone)]
pub struct ServiceControlSender {
    inner: async_mpsc::UnboundedSender<ServiceControlMessage>,
}

impl ServiceControlSender {
    pub fn send(&self, message: ServiceControlMessage) -> Result<(), ServiceControlMessage> {
        self.inner
            .unbounded_send(message)
            .map_err(|error| error.into_inner())
    }
}

#[derive(Clone)]
pub struct RunningServiceHandle {
    pub instance_id: u64,
    pub control_tx: ServiceControlSender,
}

#[derive(Clone)]
struct JobLaunch {
    req_id: u64,
    cancellation: JobCancellationToken,
    payload: Vec<u8>,
    on_ok: Option<ActionEnvelope>,
    on_err: Option<ActionEnvelope>,
    resource: Option<ResourceExecutionContext>,
    tx: mpsc::Sender<AsyncMessage>,
    wake: WakeFn,
}

#[derive(Clone)]
struct CapabilityLaunch {
    req_id: u64,
    payload: Vec<u8>,
    on_ok: Option<ActionEnvelope>,
    on_err: Option<ActionEnvelope>,
    resource: Option<ResourceExecutionContext>,
    tx: mpsc::Sender<AsyncMessage>,
    wake: WakeFn,
}

#[derive(Clone)]
struct ServiceLaunch {
    service_name: String,
    slot_key: String,
    instance_id: u64,
    config: Vec<u8>,
    resource: Option<ResourceExecutionContext>,
    tx: mpsc::Sender<AsyncMessage>,
    wake: WakeFn,
}

type JobHandler = dyn Fn(JobLaunch) + Send + Sync;
type ServiceSpawner = dyn Fn(ServiceLaunch) -> RunningServiceHandle + Send + Sync;
type CapabilitySpawner = dyn Fn(CapabilityLaunch) + Send + Sync;

pub struct AsyncRegistry {
    jobs: HashMap<String, Arc<JobHandler>>,
    services: HashMap<String, Arc<ServiceSpawner>>,
    operations: HashMap<String, Arc<CapabilitySpawner>>,
    data_streams: DataStreamRegistry,
    job_cancellations: Arc<Mutex<HashMap<u64, JobCancellationToken>>>,
}

impl Default for AsyncRegistry {
    fn default() -> Self {
        Self {
            jobs: HashMap::new(),
            services: HashMap::new(),
            operations: HashMap::new(),
            data_streams: DataStreamRegistry::new(),
            job_cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AsyncRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_operation_capability<C: OperationCapability>(
        &self,
        capability: CapabilityType<C>,
    ) -> bool {
        self.operations.contains_key(capability.name)
    }

    pub fn register_operation_capability<C, F, Fut>(
        &mut self,
        capability: CapabilityType<C>,
        handler: F,
    ) where
        C: OperationCapability,
        F: Fn(C::Request, CapabilityCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<C::Ok, C::Err>> + 'static,
    {
        let handler = Arc::new(handler);
        let data_streams = self.data_streams.clone();
        self.operations.insert(
            capability.name.to_string(),
            Arc::new(move |launch: CapabilityLaunch| {
                let name = capability.name.to_string();
                let handler = handler.clone();
                let data_streams = data_streams.clone();
                let request = match serde_json::from_slice::<C::Request>(&launch.payload) {
                    Ok(request) => request,
                    Err(err) => {
                        send_capability_err(name, launch, None, Some(err.to_string()));
                        return;
                    }
                };

                wasm_bindgen_futures::spawn_local(async move {
                    match handler(
                        request,
                        CapabilityCtx::new_runtime(launch.req_id, data_streams),
                    )
                    .await
                    {
                        Ok(ok) => match serde_json::to_vec(&ok) {
                            Ok(payload) => {
                                let _ = launch.tx.send(AsyncMessage::CapabilityOk {
                                    capability_name: name,
                                    req_id: launch.req_id,
                                    payload,
                                    on_ok: launch.on_ok,
                                    resource: launch.resource,
                                });
                                (launch.wake)();
                            }
                            Err(err) => {
                                send_capability_err(name, launch, None, Some(err.to_string()))
                            }
                        },
                        Err(err) => {
                            let (payload, message) = serde_json::to_vec(&err)
                                .ok()
                                .map(|payload| (Some(payload), None))
                                .unwrap_or_else(|| {
                                    (None, Some("capability error serialization failed".into()))
                                });
                            send_capability_err(name, launch, payload, message);
                        }
                    }
                });
            }),
        );
    }

    pub fn register_job<J, F, Fut>(&mut self, job: JobRef<J>, handler: F)
    where
        J: JobSpec,
        F: Fn(J::Request, JobCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<J::Ok, J::Err>> + 'static,
    {
        let handler = Arc::new(handler);
        let data_streams = self.data_streams.clone();
        self.jobs.insert(
            job.name.to_string(),
            Arc::new(move |launch: JobLaunch| {
                let handler = handler.clone();
                let data_streams = data_streams.clone();
                let request = match serde_json::from_slice::<J::Request>(&launch.payload) {
                    Ok(request) => request,
                    Err(err) => {
                        send_job_err::<J>(launch, None, Some(err.to_string()));
                        return;
                    }
                };

                wasm_bindgen_futures::spawn_local(async move {
                    match handler(
                        request,
                        JobCtx::new_runtime_with_cancellation(
                            launch.req_id,
                            data_streams,
                            launch.cancellation.clone(),
                        ),
                    )
                    .await
                    {
                        Ok(ok) => match serde_json::to_vec(&ok) {
                            Ok(payload) => {
                                let _ = launch.tx.send(AsyncMessage::JobOk {
                                    job_name: J::NAME.to_string(),
                                    req_id: launch.req_id,
                                    payload,
                                    on_ok: launch.on_ok,
                                    resource: launch.resource,
                                });
                                (launch.wake)();
                            }
                            Err(err) => send_job_err::<J>(launch, None, Some(err.to_string())),
                        },
                        Err(err) => {
                            let (payload, message) = serde_json::to_vec(&err)
                                .ok()
                                .map(|payload| (Some(payload), None))
                                .unwrap_or_else(|| {
                                    (None, Some("job error serialization failed".into()))
                                });
                            send_job_err::<J>(launch, payload, message);
                        }
                    }
                });
            }),
        );
    }

    pub fn register_service<S, F, Fut>(&mut self, service: ServiceType<S>, starter: F)
    where
        S: ServiceSpec + 'static,
        F: Fn(S::Config, fission_core::ServiceCtx<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Box<dyn ServiceRunner<S>>, S::StartErr>> + 'static,
    {
        let starter = Arc::new(starter);
        let data_streams = self.data_streams.clone();
        self.services.insert(
            service.name.to_string(),
            Arc::new(move |launch: ServiceLaunch| {
                let (control_tx, mut control_rx) = async_mpsc::unbounded();
                let starter = starter.clone();
                let data_streams = data_streams.clone();
                let service_name = launch.service_name.clone();
                let slot_key = launch.slot_key.clone();
                let instance_id = launch.instance_id;
                let resource = launch.resource.clone();
                let tx = launch.tx.clone();
                let wake = launch.wake.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let config = match serde_json::from_slice::<S::Config>(&launch.config) {
                        Ok(config) => config,
                        Err(error) => {
                            let _ = tx.send(AsyncMessage::ServiceStartFailed {
                                service_name,
                                slot_key,
                                instance_id,
                                payload: None,
                                message: Some(error.to_string()),
                                resource,
                            });
                            wake();
                            return;
                        }
                    };

                    let tx_for_emit = tx.clone();
                    let wake_for_emit = wake.clone();
                    let service_name_for_emit = service_name.clone();
                    let slot_key_for_emit = slot_key.clone();
                    let resource_for_emit = resource.clone();
                    let emit = Arc::new(move |payload: Vec<u8>| {
                        let tx = tx_for_emit.clone();
                        let wake = wake_for_emit.clone();
                        let service_name = service_name_for_emit.clone();
                        let slot_key = slot_key_for_emit.clone();
                        let resource = resource_for_emit.clone();
                        Box::pin(async move {
                            tx.send(AsyncMessage::ServiceEvent {
                                service_name,
                                slot_key,
                                instance_id,
                                payload,
                                resource,
                            })
                            .map_err(|error| error.to_string())?;
                            wake();
                            Ok(())
                        }) as fission_core::BoxFuture<Result<(), String>>
                    });
                    let ctx = ServiceCtx::<S>::new_runtime(
                        service_name.clone(),
                        slot_key.clone(),
                        instance_id,
                        data_streams,
                        emit,
                    );

                    let mut runner = match starter(config, ctx.clone()).await {
                        Ok(runner) => {
                            let _ = tx.send(AsyncMessage::ServiceStarted {
                                service_name: service_name.clone(),
                                slot_key: slot_key.clone(),
                                instance_id,
                                resource: resource.clone(),
                            });
                            wake();
                            runner
                        }
                        Err(error) => {
                            let (payload, message) = serde_json::to_vec(&error)
                                .ok()
                                .map(|payload| (Some(payload), None))
                                .unwrap_or_else(|| {
                                    (
                                        None,
                                        Some("service start error serialization failed".into()),
                                    )
                                });
                            let _ = tx.send(AsyncMessage::ServiceStartFailed {
                                service_name,
                                slot_key,
                                instance_id,
                                payload,
                                message,
                                resource,
                            });
                            wake();
                            return;
                        }
                    };

                    while let Some(message) = control_rx.next().await {
                        match message {
                            ServiceControlMessage::Command {
                                req_id,
                                payload,
                                on_ok,
                                on_err,
                            } => {
                                let command = match serde_json::from_slice::<S::Command>(&payload) {
                                    Ok(command) => command,
                                    Err(error) => {
                                        let _ = tx.send(AsyncMessage::ServiceCommandErr {
                                            service_name: service_name.clone(),
                                            slot_key: slot_key.clone(),
                                            instance_id,
                                            req_id,
                                            payload: None,
                                            on_err,
                                            message: Some(error.to_string()),
                                            resource: resource.clone(),
                                        });
                                        wake();
                                        continue;
                                    }
                                };

                                match runner.on_command(command, ctx.clone()).await {
                                    Ok(result) => {
                                        let payload = serde_json::to_vec(&result).ok();
                                        let _ = tx.send(AsyncMessage::ServiceCommandOk {
                                            service_name: service_name.clone(),
                                            slot_key: slot_key.clone(),
                                            instance_id,
                                            req_id,
                                            payload,
                                            on_ok,
                                            resource: resource.clone(),
                                        });
                                    }
                                    Err(error) => {
                                        let (payload, message) = serde_json::to_vec(&error)
                                            .ok()
                                            .map(|payload| (Some(payload), None))
                                            .unwrap_or_else(|| {
                                                (
                                                    None,
                                                    Some(
                                                        "service command error serialization failed"
                                                            .into(),
                                                    ),
                                                )
                                            });
                                        let _ = tx.send(AsyncMessage::ServiceCommandErr {
                                            service_name: service_name.clone(),
                                            slot_key: slot_key.clone(),
                                            instance_id,
                                            req_id,
                                            payload,
                                            on_err,
                                            message,
                                            resource: resource.clone(),
                                        });
                                    }
                                }
                                wake();
                            }
                            ServiceControlMessage::Stop => {
                                runner.on_stop(ctx.clone()).await;
                                let _ = tx.send(AsyncMessage::ServiceStopped {
                                    service_name: service_name.clone(),
                                    slot_key: slot_key.clone(),
                                    instance_id,
                                    resource: resource.clone(),
                                });
                                wake();
                                return;
                            }
                        }
                    }

                    runner.on_stop(ctx).await;
                    let _ = tx.send(AsyncMessage::ServiceStopped {
                        service_name,
                        slot_key,
                        instance_id,
                        resource,
                    });
                    wake();
                });

                RunningServiceHandle {
                    instance_id: launch.instance_id,
                    control_tx: ServiceControlSender { inner: control_tx },
                }
            }),
        );
    }

    pub fn spawn_job(
        &self,
        job_name: &str,
        inline: Option<InlineJob>,
        req_id: u64,
        payload: Vec<u8>,
        on_ok: Option<ActionEnvelope>,
        on_err: Option<ActionEnvelope>,
        resource: Option<ResourceExecutionContext>,
        tx: &mpsc::Sender<AsyncMessage>,
        wake: WakeFn,
    ) -> bool {
        let handler = if inline.is_none() {
            let Some(handler) = self.jobs.get(job_name) else {
                return false;
            };
            Some(handler.clone())
        } else {
            None
        };
        let cancellation = JobCancellationToken::new();
        self.job_cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(req_id, cancellation.clone());
        let launch = JobLaunch {
            req_id,
            cancellation,
            payload,
            on_ok,
            on_err,
            resource,
            tx: tx.clone(),
            wake,
        };
        let Some(inline) = inline else {
            let Some(handler) = handler else {
                return false;
            };
            handler(launch);
            return true;
        };
        let data_streams = self.data_streams.clone();
        let name = job_name.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            match inline
                .run(
                    launch.payload,
                    JobCtx::new_runtime_with_cancellation(
                        launch.req_id,
                        data_streams,
                        launch.cancellation.clone(),
                    ),
                )
                .await
            {
                Ok(payload) => {
                    let _ = launch.tx.send(AsyncMessage::JobOk {
                        job_name: name,
                        req_id: launch.req_id,
                        payload,
                        on_ok: launch.on_ok,
                        resource: launch.resource,
                    });
                }
                Err(error) => {
                    let _ = launch.tx.send(AsyncMessage::JobErr {
                        job_name: name,
                        req_id: launch.req_id,
                        payload: error.payload,
                        on_err: launch.on_err,
                        message: error.message,
                        resource: launch.resource,
                    });
                }
            }
            (launch.wake)();
        });
        true
    }

    /// Requests cooperative cancellation of the app job with this request id.
    ///
    /// The result stays registered until its completion message is drained, so
    /// the shell can suppress a completion that races with cancellation.
    pub fn cancel_job(&self, req_id: u64) -> bool {
        let cancellations = self
            .job_cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(cancellation) = cancellations.get(&req_id) else {
            return false;
        };
        cancellation.cancel();
        true
    }

    /// Removes the completed job registration and reports whether it was cancelled.
    pub fn finish_job(&self, req_id: u64) -> bool {
        self.job_cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&req_id)
            .is_some_and(|cancellation| cancellation.is_cancelled())
    }

    pub fn spawn_capability(
        &self,
        capability_name: &str,
        req_id: u64,
        payload: Vec<u8>,
        on_ok: Option<ActionEnvelope>,
        on_err: Option<ActionEnvelope>,
        resource: Option<ResourceExecutionContext>,
        tx: &mpsc::Sender<AsyncMessage>,
        wake: WakeFn,
    ) -> bool {
        let Some(handler) = self.operations.get(capability_name) else {
            return false;
        };
        handler(CapabilityLaunch {
            req_id,
            payload,
            on_ok,
            on_err,
            resource,
            tx: tx.clone(),
            wake,
        });
        true
    }

    pub fn spawn_service(
        &self,
        service_name: &str,
        slot_key: &str,
        instance_id: u64,
        config: Vec<u8>,
        resource: Option<ResourceExecutionContext>,
        tx: &mpsc::Sender<AsyncMessage>,
        wake: WakeFn,
    ) -> Option<RunningServiceHandle> {
        let spawner = self.services.get(service_name)?;
        Some(spawner(ServiceLaunch {
            service_name: service_name.to_string(),
            slot_key: slot_key.to_string(),
            instance_id,
            config,
            resource,
            tx: tx.clone(),
            wake,
        }))
    }
}

fn send_job_err<J: JobSpec>(launch: JobLaunch, payload: Option<Vec<u8>>, message: Option<String>) {
    let _ = launch.tx.send(AsyncMessage::JobErr {
        job_name: J::NAME.to_string(),
        req_id: launch.req_id,
        payload,
        on_err: launch.on_err,
        message,
        resource: launch.resource,
    });
    (launch.wake)();
}

fn send_capability_err(
    capability_name: String,
    launch: CapabilityLaunch,
    payload: Option<Vec<u8>>,
    message: Option<String>,
) {
    let _ = launch.tx.send(AsyncMessage::CapabilityErr {
        capability_name,
        req_id: launch.req_id,
        payload,
        on_err: launch.on_err,
        message,
        resource: launch.resource,
    });
    (launch.wake)();
}

#[cfg(test)]
mod tests {
    use super::{
        AsyncMessage, AsyncRegistry, ServiceControlMessage, ServiceSpec, ServiceType, WakeFn,
    };
    use fission_core::{BoxFuture, ServiceCtx, ServiceRunner, ServiceSlot};
    use futures_util::StreamExt;
    use serde::{Deserialize, Serialize};
    use std::sync::{mpsc, Arc};
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    wasm_bindgen_test_configure!(run_in_browser);

    struct EchoService;

    #[derive(Serialize, Deserialize)]
    struct EchoConfig;

    #[derive(Serialize, Deserialize)]
    enum EchoCommand {
        Echo(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    enum EchoEvent {
        Ready,
        Echoed(String),
    }

    impl ServiceSpec for EchoService {
        type Config = EchoConfig;
        type Command = EchoCommand;
        type CommandOk = String;
        type CommandErr = String;
        type Event = EchoEvent;
        type StartErr = String;
        const NAME: &'static str = "wasm-echo-service";
    }

    const ECHO_SERVICE: ServiceType<EchoService> = ServiceType::new("wasm-echo-service");

    struct EchoRunner;

    impl ServiceRunner<EchoService> for EchoRunner {
        fn on_command(
            &mut self,
            command: EchoCommand,
            ctx: ServiceCtx<EchoService>,
        ) -> BoxFuture<Result<String, String>> {
            Box::pin(async move {
                let EchoCommand::Echo(value) = command;
                ctx.emit(EchoEvent::Echoed(value.clone()))
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(value)
            })
        }

        fn on_stop(self: Box<Self>, _ctx: ServiceCtx<EchoService>) -> BoxFuture<()> {
            Box::pin(async {})
        }
    }

    async fn next_message(
        messages: &mpsc::Receiver<AsyncMessage>,
        wake_rx: &mut futures_channel::mpsc::UnboundedReceiver<()>,
    ) -> AsyncMessage {
        loop {
            let _ = wake_rx.next().await;
            match messages.try_recv() {
                Ok(message) => return message,
                Err(mpsc::TryRecvError::Empty) => continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("service message channel disconnected")
                }
            }
        }
    }

    #[wasm_bindgen_test]
    async fn wasm_services_start_emit_handle_commands_and_stop() {
        let mut registry = AsyncRegistry::new();
        registry.register_service(
            ECHO_SERVICE,
            |_config: EchoConfig, ctx: ServiceCtx<EchoService>| async move {
                ctx.emit(EchoEvent::Ready)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>(Box::new(EchoRunner) as Box<dyn ServiceRunner<EchoService>>)
            },
        );

        let (message_tx, message_rx) = mpsc::channel();
        let (wake_tx, mut wake_rx) = futures_channel::mpsc::unbounded();
        let wake: WakeFn = Arc::new(move || {
            let _ = wake_tx.unbounded_send(());
        });
        let handle = registry
            .spawn_service(
                ECHO_SERVICE.name,
                ServiceSlot::singleton(ECHO_SERVICE).slot_key(),
                7,
                serde_json::to_vec(&EchoConfig).unwrap(),
                None,
                &message_tx,
                wake,
            )
            .expect("registered wasm service");

        let first = next_message(&message_rx, &mut wake_rx).await;
        let second = next_message(&message_rx, &mut wake_rx).await;
        assert!(
            matches!(first, AsyncMessage::ServiceStarted { instance_id: 7, .. })
                || matches!(second, AsyncMessage::ServiceStarted { instance_id: 7, .. })
        );
        assert!(
            matches!(first, AsyncMessage::ServiceEvent { payload, .. } if serde_json::from_slice::<EchoEvent>(&payload).ok() == Some(EchoEvent::Ready))
                || matches!(second, AsyncMessage::ServiceEvent { payload, .. } if serde_json::from_slice::<EchoEvent>(&payload).ok() == Some(EchoEvent::Ready))
        );

        assert!(handle
            .control_tx
            .send(ServiceControlMessage::Command {
                req_id: 9,
                payload: serde_json::to_vec(&EchoCommand::Echo("hello".into())).unwrap(),
                on_ok: None,
                on_err: None,
            })
            .is_ok());

        let first = next_message(&message_rx, &mut wake_rx).await;
        let second = next_message(&message_rx, &mut wake_rx).await;
        assert!(
            matches!(&first, AsyncMessage::ServiceCommandOk { req_id: 9, payload: Some(payload), .. } if serde_json::from_slice::<String>(payload).ok().as_deref() == Some("hello"))
                || matches!(&second, AsyncMessage::ServiceCommandOk { req_id: 9, payload: Some(payload), .. } if serde_json::from_slice::<String>(payload).ok().as_deref() == Some("hello"))
        );
        assert!(
            matches!(first, AsyncMessage::ServiceEvent { payload, .. } if serde_json::from_slice::<EchoEvent>(&payload).ok() == Some(EchoEvent::Echoed("hello".into())))
                || matches!(second, AsyncMessage::ServiceEvent { payload, .. } if serde_json::from_slice::<EchoEvent>(&payload).ok() == Some(EchoEvent::Echoed("hello".into())))
        );

        assert!(handle.control_tx.send(ServiceControlMessage::Stop).is_ok());
        assert!(matches!(
            next_message(&message_rx, &mut wake_rx).await,
            AsyncMessage::ServiceStopped { instance_id: 7, .. }
        ));
    }
}
