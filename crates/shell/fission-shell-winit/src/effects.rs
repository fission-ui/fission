use super::*;

/// Drain pending effects from the runtime, delegating capability work to the
/// async registry and runtime-control effects to the shell/runtime boundary.
///
/// Returns `true` if any synchronous callback was dispatched (caller should redraw).

pub(super) fn process_pending_effects(
    runtime: &mut Runtime,
    effect_tx: &mpsc::Sender<AsyncMessage>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
) -> bool {
    let pending = std::mem::take(&mut runtime.pending_effects);
    if pending.is_empty() {
        return false;
    }

    let mut dispatched_callback = false;
    let wake = {
        let proxy = Arc::new(Mutex::new(event_proxy.clone()));
        Arc::new(move || {
            if let Ok(proxy) = proxy.lock() {
                let _ = proxy.send_event(TestEvent::Wake);
            }
        })
    };

    for env in pending {
        match env.effect {
            Effect::Runtime(ref runtime_effect) => {
                diag::emit(
                    diag::DiagCategory::Input,
                    diag::DiagLevel::Debug,
                    diag::DiagEventKind::InputEvent {
                        kind: format!("runtime_effect:{:?}", runtime_effect),
                        target: None,
                        position: None,
                    },
                );
                if runtime.queue_runtime_effect(runtime_effect.clone()) {
                    dispatched_callback = true;
                    (wake)();
                }
            }
            Effect::Capability(capability) => match capability {
                CapabilityInvocationPayload::Operation(op) => {
                    if !async_registry.spawn_capability(
                        &op.capability_name,
                        env.req_id,
                        op.request,
                        env.on_ok.clone(),
                        env.on_err.clone(),
                        env.resource.clone(),
                        effect_tx,
                        wake.clone(),
                    ) {
                        let _ = effect_tx.send(AsyncMessage::CapabilityErr {
                            capability_name: op.capability_name,
                            req_id: env.req_id,
                            payload: None,
                            on_err: env.on_err.clone(),
                            message: Some(
                                "no async operation capability handler registered".into(),
                            ),
                            resource: env.resource.clone(),
                        });
                        (wake)();
                    }
                }
            },
            Effect::Job(job) => {
                if !async_registry.spawn_job(
                    &job.job_name,
                    env.req_id,
                    job.payload,
                    env.on_ok.clone(),
                    env.on_err.clone(),
                    env.resource.clone(),
                    effect_tx,
                    wake.clone(),
                ) {
                    let _ = effect_tx.send(AsyncMessage::JobErr {
                        job_name: job.job_name,
                        req_id: env.req_id,
                        payload: None,
                        on_err: env.on_err.clone(),
                        message: Some("no async job handler registered".into()),
                        resource: env.resource.clone(),
                    });
                    (wake)();
                }
            }
            Effect::StartService(start) => {
                let key = (start.service_name.clone(), start.slot_key.clone());
                if let Some(previous) = active_services.remove(&key) {
                    let _ = previous
                        .runtime
                        .control_tx
                        .send(ServiceControlMessage::Stop);
                }

                let instance_id = *next_service_instance_id;
                *next_service_instance_id = next_service_instance_id.saturating_add(1);
                let bindings = env.service_bindings.clone().unwrap_or_default();
                service_bindings.insert(
                    (
                        start.service_name.clone(),
                        start.slot_key.clone(),
                        instance_id,
                    ),
                    bindings,
                );

                match async_registry.spawn_service(
                    &start.service_name,
                    &start.slot_key,
                    instance_id,
                    start.config,
                    env.resource.clone(),
                    effect_tx,
                    wake.clone(),
                ) {
                    Some(handle) => {
                        active_services.insert(key, ActiveServiceHandle { runtime: handle });
                    }
                    None => {
                        let _ = service_bindings.remove(&(
                            start.service_name.clone(),
                            start.slot_key.clone(),
                            instance_id,
                        ));
                        let _ = effect_tx.send(AsyncMessage::ServiceStartFailed {
                            service_name: start.service_name,
                            slot_key: start.slot_key,
                            instance_id,
                            payload: None,
                            message: Some("no async service handler registered".into()),
                            resource: env.resource.clone(),
                        });
                        (wake)();
                    }
                }
            }
            Effect::ServiceCommand(command) => {
                let key = (command.service_name.clone(), command.slot_key.clone());
                if let Some(handle) = active_services.get(&key) {
                    let _ = handle
                        .runtime
                        .control_tx
                        .send(ServiceControlMessage::Command {
                            req_id: env.req_id,
                            payload: command.payload,
                            on_ok: env.on_ok.clone(),
                            on_err: env.on_err.clone(),
                        });
                } else {
                    let _ = effect_tx.send(AsyncMessage::ServiceCommandErr {
                        service_name: command.service_name,
                        slot_key: command.slot_key,
                        instance_id: 0,
                        req_id: env.req_id,
                        payload: None,
                        on_err: env.on_err.clone(),
                        message: Some("service is not running".into()),
                        resource: env.resource.clone(),
                    });
                    (wake)();
                }
            }
            Effect::StopService(stop) => {
                let key = (stop.service_name.clone(), stop.slot_key.clone());
                if let Some(handle) = active_services.remove(&key) {
                    let _ = handle.runtime.control_tx.send(ServiceControlMessage::Stop);
                }
            }
        }
    }

    dispatched_callback
}

/// Drain completed background effect results from the channel and dispatch
/// their continuations on the main thread.
///
/// Returns `true` if any continuation was dispatched (caller should redraw).
pub(super) fn drain_effect_results(
    runtime: &mut Runtime,
    effect_rx: &mpsc::Receiver<AsyncMessage>,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
) -> bool {
    let mut dispatched = false;

    while let Ok(message) = effect_rx.try_recv() {
        match message {
            AsyncMessage::JobOk {
                job_name,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::JobOk {
                            job_name,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::JobErr {
                job_name,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::JobErr {
                            job_name,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::ServiceStarted {
                service_name,
                slot_key,
                instance_id,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(bindings) =
                    service_bindings.get(&(service_name.clone(), slot_key.clone(), instance_id))
                {
                    if let Some(action) = bindings.on_started.clone() {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceStarted {
                                service_name,
                                slot_key,
                                instance_id,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceStartFailed {
                service_name,
                slot_key,
                instance_id,
                payload,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        service_bindings.remove(&(service_name, slot_key, instance_id));
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let should_dispatch = active_services
                    .get(&key)
                    .map(|current| current.runtime.instance_id == instance_id)
                    .unwrap_or(true);
                active_services.remove(&key);
                let bindings =
                    service_bindings.remove(&(service_name.clone(), slot_key.clone(), instance_id));
                if should_dispatch {
                    if let Some(action) = bindings.and_then(|bindings| bindings.on_start_failed) {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceStartFailed {
                                service_name,
                                slot_key,
                                payload,
                                message,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceEvent {
                service_name,
                slot_key,
                instance_id,
                payload,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(bindings) =
                    service_bindings.get(&(service_name.clone(), slot_key.clone(), instance_id))
                {
                    if let Some(action) = bindings.on_event.clone() {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceEvent {
                                service_name,
                                slot_key,
                                instance_id,
                                payload,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceStopped {
                service_name,
                slot_key,
                instance_id,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        service_bindings.remove(&(service_name, slot_key, instance_id));
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let should_dispatch = active_services
                    .get(&key)
                    .map(|current| current.runtime.instance_id == instance_id)
                    .unwrap_or(true);
                if should_dispatch {
                    active_services.remove(&key);
                }
                let bindings =
                    service_bindings.remove(&(service_name.clone(), slot_key.clone(), instance_id));
                if should_dispatch {
                    if let Some(action) = bindings.and_then(|bindings| bindings.on_stopped) {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceStopped {
                                service_name,
                                slot_key,
                                instance_id,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceCommandOk {
                service_name,
                slot_key,
                instance_id,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::ServiceCommandOk {
                            service_name,
                            slot_key,
                            instance_id,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::ServiceCommandErr {
                service_name,
                slot_key,
                instance_id,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                if instance_id != 0 {
                    let Some(current) = active_services.get(&key) else {
                        continue;
                    };
                    if current.runtime.instance_id != instance_id {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::ServiceCommandErr {
                            service_name,
                            slot_key,
                            instance_id,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::CapabilityOk {
                capability_name,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::CapabilityOk {
                            capability: capability_name,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::CapabilityErr {
                capability_name,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::CapabilityErr {
                            capability: capability_name,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
        }
    }

    dispatched
}
