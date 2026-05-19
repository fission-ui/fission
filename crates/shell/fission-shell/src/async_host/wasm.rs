use fission_core::{
    ActionEnvelope, CapabilityCtx, CapabilityType, JobCtx, JobRef, JobSpec, OperationCapability,
    ResourceExecutionContext, ServiceSpec, ServiceType,
};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{mpsc, Arc};

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
pub struct RunningServiceHandle {
    pub instance_id: u64,
    pub control_tx: mpsc::Sender<ServiceControlMessage>,
}

#[derive(Clone)]
struct JobLaunch {
    req_id: u64,
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
}

impl Default for AsyncRegistry {
    fn default() -> Self {
        Self {
            jobs: HashMap::new(),
            services: HashMap::new(),
            operations: HashMap::new(),
        }
    }
}

impl AsyncRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_operation_capability<C, F, Fut>(
        &mut self,
        capability: CapabilityType<C>,
        handler: F,
    ) where
        C: OperationCapability,
        F: Fn(C::Request, CapabilityCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<C::Ok, C::Err>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        self.operations.insert(
            capability.name.to_string(),
            Arc::new(move |launch: CapabilityLaunch| {
                let name = capability.name.to_string();
                let handler = handler.clone();
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
                        CapabilityCtx {
                            req_id: launch.req_id,
                        },
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
        Fut: Future<Output = Result<J::Ok, J::Err>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        self.jobs.insert(
            job.name.to_string(),
            Arc::new(move |launch: JobLaunch| {
                let handler = handler.clone();
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
                        JobCtx {
                            req_id: launch.req_id,
                        },
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

    pub fn register_service<S, F, Fut>(&mut self, service: ServiceType<S>, _starter: F)
    where
        S: ServiceSpec + 'static,
        F: Fn(S::Config, fission_core::ServiceCtx<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Box<dyn fission_core::ServiceRunner<S>>, S::StartErr>>
            + Send
            + 'static,
    {
        self.services.insert(
            service.name.to_string(),
            Arc::new(move |launch: ServiceLaunch| {
                let (control_tx, _control_rx) = mpsc::channel();
                let _ = launch.tx.send(AsyncMessage::ServiceStartFailed {
                    service_name: launch.service_name,
                    slot_key: launch.slot_key,
                    instance_id: launch.instance_id,
                    payload: None,
                    message: Some(
                        "fission-shell services are not supported on wasm32-unknown-unknown yet"
                            .into(),
                    ),
                    resource: launch.resource,
                });
                (launch.wake)();
                RunningServiceHandle {
                    instance_id: launch.instance_id,
                    control_tx,
                }
            }),
        );
    }

    pub fn spawn_job(
        &self,
        job_name: &str,
        req_id: u64,
        payload: Vec<u8>,
        on_ok: Option<ActionEnvelope>,
        on_err: Option<ActionEnvelope>,
        resource: Option<ResourceExecutionContext>,
        tx: &mpsc::Sender<AsyncMessage>,
        wake: WakeFn,
    ) -> bool {
        let Some(handler) = self.jobs.get(job_name) else {
            return false;
        };
        handler(JobLaunch {
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
        _config: Vec<u8>,
        resource: Option<ResourceExecutionContext>,
        tx: &mpsc::Sender<AsyncMessage>,
        wake: WakeFn,
    ) -> Option<RunningServiceHandle> {
        let spawner = self.services.get(service_name)?;
        Some(spawner(ServiceLaunch {
            service_name: service_name.to_string(),
            slot_key: slot_key.to_string(),
            instance_id,
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
