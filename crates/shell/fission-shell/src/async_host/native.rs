use fission_core::{
    ActionEnvelope, BoxFuture, CapabilityCtx, CapabilityType, JobCtx, JobRef, JobSpec,
    OperationCapability, ResourceExecutionContext, ServiceCtx, ServiceRunner, ServiceSpec,
    ServiceType,
};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{mpsc, Arc};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime};

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
                let handler = handler.clone();
                let name = capability.name.to_string();
                std::thread::spawn(move || {
                    let runtime = match new_job_runtime() {
                        Ok(runtime) => runtime,
                        Err(err) => {
                            let _ = launch.tx.send(AsyncMessage::CapabilityErr {
                                capability_name: name,
                                req_id: launch.req_id,
                                payload: None,
                                on_err: launch.on_err,
                                message: Some(err),
                                resource: launch.resource,
                            });
                            (launch.wake)();
                            return;
                        }
                    };

                    let request = match serde_json::from_slice::<C::Request>(&launch.payload) {
                        Ok(request) => request,
                        Err(err) => {
                            let _ = launch.tx.send(AsyncMessage::CapabilityErr {
                                capability_name: name,
                                req_id: launch.req_id,
                                payload: None,
                                on_err: launch.on_err,
                                message: Some(err.to_string()),
                                resource: launch.resource,
                            });
                            (launch.wake)();
                            return;
                        }
                    };

                    match runtime.block_on(handler(
                        request,
                        CapabilityCtx {
                            req_id: launch.req_id,
                        },
                    )) {
                        Ok(ok) => match serde_json::to_vec(&ok) {
                            Ok(payload) => {
                                let _ = launch.tx.send(AsyncMessage::CapabilityOk {
                                    capability_name: name,
                                    req_id: launch.req_id,
                                    payload,
                                    on_ok: launch.on_ok,
                                    resource: launch.resource,
                                });
                            }
                            Err(err) => {
                                let _ = launch.tx.send(AsyncMessage::CapabilityErr {
                                    capability_name: name,
                                    req_id: launch.req_id,
                                    payload: None,
                                    on_err: launch.on_err,
                                    message: Some(err.to_string()),
                                    resource: launch.resource,
                                });
                            }
                        },
                        Err(err) => {
                            let (payload, message) = serde_json::to_vec(&err)
                                .ok()
                                .map(|payload| (Some(payload), None))
                                .unwrap_or_else(|| {
                                    (None, Some("capability error serialization failed".into()))
                                });
                            let _ = launch.tx.send(AsyncMessage::CapabilityErr {
                                capability_name: name,
                                req_id: launch.req_id,
                                payload,
                                on_err: launch.on_err,
                                message,
                                resource: launch.resource,
                            });
                        }
                    }

                    (launch.wake)();
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
                std::thread::spawn(move || {
                    let runtime = match new_job_runtime() {
                        Ok(runtime) => runtime,
                        Err(err) => {
                            let _ = launch.tx.send(AsyncMessage::JobErr {
                                job_name: J::NAME.to_string(),
                                req_id: launch.req_id,
                                payload: None,
                                on_err: launch.on_err,
                                message: Some(err),
                                resource: launch.resource,
                            });
                            (launch.wake)();
                            return;
                        }
                    };
                    let request = match serde_json::from_slice::<J::Request>(&launch.payload) {
                        Ok(request) => request,
                        Err(err) => {
                            let _ = launch.tx.send(AsyncMessage::JobErr {
                                job_name: J::NAME.to_string(),
                                req_id: launch.req_id,
                                payload: None,
                                on_err: launch.on_err,
                                message: Some(err.to_string()),
                                resource: launch.resource,
                            });
                            (launch.wake)();
                            return;
                        }
                    };

                    match runtime.block_on(handler(
                        request,
                        JobCtx {
                            req_id: launch.req_id,
                        },
                    )) {
                        Ok(ok) => match serde_json::to_vec(&ok) {
                            Ok(payload) => {
                                let _ = launch.tx.send(AsyncMessage::JobOk {
                                    job_name: J::NAME.to_string(),
                                    req_id: launch.req_id,
                                    payload,
                                    on_ok: launch.on_ok,
                                    resource: launch.resource,
                                });
                            }
                            Err(err) => {
                                let _ = launch.tx.send(AsyncMessage::JobErr {
                                    job_name: J::NAME.to_string(),
                                    req_id: launch.req_id,
                                    payload: None,
                                    on_err: launch.on_err,
                                    message: Some(err.to_string()),
                                    resource: launch.resource,
                                });
                            }
                        },
                        Err(err) => {
                            let (payload, message) = serde_json::to_vec(&err)
                                .ok()
                                .map(|payload| (Some(payload), None))
                                .unwrap_or_else(|| {
                                    (None, Some("job error serialization failed".into()))
                                });
                            let _ = launch.tx.send(AsyncMessage::JobErr {
                                job_name: J::NAME.to_string(),
                                req_id: launch.req_id,
                                payload,
                                on_err: launch.on_err,
                                message,
                                resource: launch.resource,
                            });
                        }
                    }

                    (launch.wake)();
                });
            }),
        );
    }

    pub fn register_service<S, F, Fut>(&mut self, service: ServiceType<S>, starter: F)
    where
        S: ServiceSpec + 'static,
        F: Fn(S::Config, ServiceCtx<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Box<dyn ServiceRunner<S>>, S::StartErr>> + Send + 'static,
    {
        let starter = Arc::new(starter);
        self.services.insert(
            service.name.to_string(),
            Arc::new(move |launch: ServiceLaunch| {
                let (control_tx, control_rx) = mpsc::channel();
                let starter = starter.clone();
                let tx = launch.tx.clone();
                let wake = launch.wake.clone();
                let service_name = launch.service_name.clone();
                let slot_key = launch.slot_key.clone();
                let resource = launch.resource.clone();
                let instance_id = launch.instance_id;
                let config_bytes = launch.config.clone();

                std::thread::spawn(move || {
                    let runtime = match new_service_runtime() {
                        Ok(runtime) => runtime,
                        Err(err) => {
                            let _ = tx.send(AsyncMessage::ServiceStartFailed {
                                service_name,
                                slot_key,
                                instance_id,
                                payload: None,
                                message: Some(err),
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
                    let emit = Arc::new(move |payload: Vec<u8>| -> BoxFuture<Result<(), String>> {
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
                            .map_err(|err| err.to_string())?;
                            wake();
                            Ok(())
                        })
                    });

                    let ctx = ServiceCtx::<S>::new_runtime(
                        service_name.clone(),
                        slot_key.clone(),
                        instance_id,
                        emit,
                    );

                    let config = match serde_json::from_slice::<S::Config>(&config_bytes) {
                        Ok(config) => config,
                        Err(err) => {
                            let _ = tx.send(AsyncMessage::ServiceStartFailed {
                                service_name,
                                slot_key,
                                instance_id,
                                payload: None,
                                message: Some(err.to_string()),
                                resource,
                            });
                            wake();
                            return;
                        }
                    };

                    let mut runner = match runtime.block_on(starter(config, ctx.clone())) {
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
                        Err(err) => {
                            let payload = serde_json::to_vec(&err).ok();
                            let _ = tx.send(AsyncMessage::ServiceStartFailed {
                                service_name,
                                slot_key,
                                instance_id,
                                payload,
                                message: None,
                                resource,
                            });
                            wake();
                            return;
                        }
                    };

                    while let Ok(message) = control_rx.recv() {
                        match message {
                            ServiceControlMessage::Command {
                                req_id,
                                payload,
                                on_ok,
                                on_err,
                            } => {
                                let command = match serde_json::from_slice::<S::Command>(&payload) {
                                    Ok(command) => command,
                                    Err(err) => {
                                        let _ = tx.send(AsyncMessage::ServiceCommandErr {
                                            service_name: service_name.clone(),
                                            slot_key: slot_key.clone(),
                                            instance_id,
                                            req_id,
                                            payload: None,
                                            on_err,
                                            message: Some(err.to_string()),
                                            resource: resource.clone(),
                                        });
                                        wake();
                                        continue;
                                    }
                                };

                                match runtime.block_on(runner.on_command(command, ctx.clone())) {
                                    Ok(ok) => {
                                        let payload = serde_json::to_vec(&ok).ok();
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
                                    Err(err) => {
                                        let (payload, message) = serde_json::to_vec(&err)
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
                                runtime.block_on(runner.on_stop(ctx.clone()));
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

                    runtime.block_on(runner.on_stop(ctx));
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

fn new_job_runtime() -> Result<TokioRuntime, String> {
    TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())
}

fn new_service_runtime() -> Result<TokioRuntime, String> {
    TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|err| err.to_string())
}
