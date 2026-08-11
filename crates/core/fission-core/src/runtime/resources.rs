use super::*;

impl Runtime {
    pub fn reconcile_resources(
        &mut self,
        declarations: Vec<RuntimeResourceDeclaration>,
    ) -> Result<()> {
        let now = self.clock().current_time();
        let mut existing = std::mem::take(&mut self.active_resources);
        let mut next = HashMap::new();

        for declaration in declarations {
            let key = declaration.key.clone();
            match existing.remove(&key) {
                Some(current)
                    if current.policy == declaration.policy
                        && current.deps == declaration.deps
                        && current.matches_kind(&declaration.kind) =>
                {
                    next.insert(key, current);
                }
                Some(current) if declaration.policy == ResourcePolicy::PreserveOnChange => {
                    next.insert(key, current);
                }
                Some(current) => {
                    self.stop_resource(&key, &current);
                    let replacement = self.start_resource(declaration, now);
                    next.insert(key, replacement);
                }
                None => {
                    let resource = self.start_resource(declaration, now);
                    next.insert(key, resource);
                }
            }
        }

        for (key, resource) in existing {
            self.stop_resource(&key, &resource);
        }

        self.active_resources = next;
        Ok(())
    }

    pub fn resource_generation(&self, key: &str) -> Option<u64> {
        self.active_resources
            .get(key)
            .map(|resource| resource.generation)
    }

    pub fn is_resource_current(&self, resource: &ResourceExecutionContext) -> bool {
        self.resource_generation(&resource.key) == Some(resource.generation)
    }

    pub(super) fn start_resource(
        &mut self,
        declaration: RuntimeResourceDeclaration,
        now: CurrentTime,
    ) -> ActiveResource {
        let generation = self.next_resource_generation;
        self.next_resource_generation += 1;

        let context = ResourceExecutionContext {
            key: declaration.key.clone(),
            generation,
        };

        let kind = match declaration.kind {
            RuntimeResourceKind::Job(mut job) => {
                job.effect.resource = Some(context);
                self.enqueue_effect(job.effect);
                ActiveResourceKind::Job
            }
            RuntimeResourceKind::Service(mut service) => {
                service.effect.resource = Some(context);
                let (service_name, slot_key) = match &service.effect.effect {
                    crate::Effect::StartService(payload) => {
                        (payload.service_name.clone(), payload.slot_key.clone())
                    }
                    _ => unreachable!("service resource must lower to StartService"),
                };
                self.enqueue_effect(service.effect);
                ActiveResourceKind::Service {
                    service_name,
                    slot_key,
                }
            }
            RuntimeResourceKind::Timer(timer) => self.start_timer_resource(timer, now),
        };

        ActiveResource {
            generation,
            deps: declaration.deps,
            policy: declaration.policy,
            kind,
        }
    }

    pub(super) fn start_timer_resource(
        &self,
        timer: TimerResource,
        now: CurrentTime,
    ) -> ActiveResourceKind {
        let interval_ms = timer.interval_ms.max(1);
        ActiveResourceKind::Timer {
            interval_ms,
            payload: timer.payload,
            on_tick: timer.on_tick,
            next_fire_at: if timer.immediate {
                now
            } else {
                now.saturating_add(interval_ms)
            },
        }
    }

    pub(super) fn stop_resource(&mut self, key: &str, resource: &ActiveResource) {
        if let ActiveResourceKind::Service {
            service_name,
            slot_key,
        } = &resource.kind
        {
            self.enqueue_effect(EffectEnvelope {
                req_id: 0,
                effect: crate::Effect::StopService(ServiceStopPayload {
                    service_name: service_name.clone(),
                    slot_key: slot_key.clone(),
                }),
                on_ok: None,
                on_err: None,
                service_bindings: None,
                resource: Some(ResourceExecutionContext {
                    key: key.to_string(),
                    generation: resource.generation,
                }),
            });
        }
    }
}

impl ActiveResource {
    pub(super) fn matches_kind(&self, kind: &RuntimeResourceKind) -> bool {
        matches!(
            (&self.kind, kind),
            (ActiveResourceKind::Job, RuntimeResourceKind::Job(_))
                | (
                    ActiveResourceKind::Timer { .. },
                    RuntimeResourceKind::Timer(_)
                )
                | (
                    ActiveResourceKind::Service { .. },
                    RuntimeResourceKind::Service(_)
                )
        )
    }
}
