use super::*;

impl<S, W> RunLoop<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub(super) fn handle_about_to_wait(&mut self, elwt: &EventLoopWindowTarget) {
        let Some(window) = self.platform_window.active_window_arc() else {
            elwt.set_control_flow(ControlFlow::Wait);
            return;
        };
        let window = window.as_ref();
        #[cfg(feature = "tray")]
        if let Some(tray) = self.active_tray.as_ref() {
            if tray.minimize_behavior() == tray::WindowMinimizeBehavior::HideToTray
                && window.is_visible().unwrap_or(true)
                && window.is_minimized() == Some(true)
            {
                tray::hide_window_to_tray(window, tray.app_switcher_policy());
            }
        }
        #[cfg(target_os = "android")]
        self.drain_pending_test_events(elwt);
        #[cfg(feature = "tray")]
        if let (Some(rx), Some(active)) = (self.tray_event_rx.as_ref(), self.active_tray.as_ref()) {
            while let Ok(event) = rx.try_recv() {
                match active.handle_event(event, window, &mut self.runtime) {
                    Ok(outcome) => {
                        if outcome.quit {
                            elwt.exit();
                            return;
                        }
                        if outcome.redraw {
                            self.invalidations.mark_build();
                            if process_pending_effects(
                                &mut self.runtime,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                            ) {
                                self.invalidations.mark_build();
                            }
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                "tray_menu_action",
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!("Fission tray event error: {error:?}");
                    }
                }
            }
        }
        let now = Instant::now();

        // Video Logic
        let mut surfaces = self.pipeline.video_surfaces.clone();
        let mut active_nodes = std::collections::HashSet::new();
        let mut player_surface_changed = false;

        for surface in &mut surfaces {
            active_nodes.insert(surface.widget_id);

            if let Some(state) = self
                .runtime
                .runtime_state
                .video
                .states
                .get(&surface.widget_id)
            {
                let source = state.asset_source.clone();
                let audio = state.audio.clone();
                let needs_player = self
                    .players
                    .get(&surface.widget_id)
                    .map(|active| active.source != source || active.audio != audio)
                    .unwrap_or(true);
                if source.is_empty() {
                    if self.players.remove(&surface.widget_id).is_some()
                        || state.surface_id.is_some()
                    {
                        if let Some(state) = self
                            .runtime
                            .runtime_state
                            .video
                            .states
                            .get_mut(&surface.widget_id)
                        {
                            state.surface_id = None;
                        }
                        player_surface_changed = true;
                    }
                } else if needs_player {
                    let player = self.video_backend.create_player(&source, &audio);
                    surface.surface_id = player.surface_id();
                    if let Some(state) = self
                        .runtime
                        .runtime_state
                        .video
                        .states
                        .get_mut(&surface.widget_id)
                    {
                        state.surface_id = Some(surface.surface_id);
                        state.duration_ms = None;
                        state.position_ms = 0;
                    }
                    player_surface_changed = true;
                    self.players.insert(
                        surface.widget_id,
                        ActivePlayer {
                            player,
                            source,
                            audio,
                            last_status: None,
                            last_rate: None,
                            last_volume: None,
                            last_muted: None,
                        },
                    );
                }
            }
            if let Some(active_player) = self.players.get(&surface.widget_id) {
                surface.surface_id = active_player.player.surface_id();
            }
        }

        // Cleanup inactive self.players
        self.players.retain(|id, _| active_nodes.contains(id));

        // Player creation changes producer availability. External placement is
        // committed only after a matching target frame presents, so schedule
        // the follow-up frame even for a paused video's cold start.
        if player_surface_changed {
            self.invalidations.mark_composite();
            request_redraw_logged(
                window,
                elwt,
                &mut self.last_redraw_at,
                self.min_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                "video_surface_ready",
            );
        }

        // Video Logic - Process Player Events and Sync State
        for (widget_id, active_player) in self.players.iter_mut() {
            if let Some(video_state) = self.runtime.runtime_state.video.states.get_mut(widget_id) {
                let player = &mut active_player.player;

                // Sync player controls from self.runtime state
                if active_player.last_status != Some(video_state.status) {
                    match video_state.status {
                        VideoStatus::Playing => player.play(),
                        VideoStatus::Paused => player.pause(),
                        VideoStatus::Stopped => player.stop(),
                        _ => {}
                    }
                    active_player.last_status = Some(video_state.status);
                }

                // Update self.runtime state from player events
                for event in player.poll_events() {
                    match event {
                        VideoEvent::Ready { duration } => {
                            video_state.duration_ms = Some(duration);
                            if video_state.status == VideoStatus::Playing {
                                player.play();
                            }
                        }
                        VideoEvent::Ended => {
                            if video_state.looped {
                                player.seek_to(0);
                                player.play();
                                video_state.status = VideoStatus::Playing;
                                video_state.pending_seek = None;
                                active_player.last_status = None;
                            } else {
                                video_state.status = VideoStatus::Ended;
                                active_player.last_status = Some(VideoStatus::Ended);
                            }
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                "video_ended",
                            );
                        }
                        VideoEvent::Error(e) => {
                            eprintln!("Video playback error for {:?}: {:?}", widget_id, e);
                            video_state.status = VideoStatus::Error;
                            active_player.last_status = Some(VideoStatus::Error);
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                "video_error",
                            );
                        }
                    }
                }
                // Sync other properties
                video_state.position_ms = player.position();

                if active_player.last_rate != Some(video_state.rate) {
                    player.set_rate(video_state.rate);
                    active_player.last_rate = Some(video_state.rate);
                }
                if active_player.last_volume != Some(video_state.volume) {
                    player.set_volume(video_state.volume);
                    active_player.last_volume = Some(video_state.volume);
                }
                if active_player.last_muted != Some(video_state.muted) {
                    player.set_muted(video_state.muted);
                    active_player.last_muted = Some(video_state.muted);
                }

                if let Some(seek_pos) = video_state.pending_seek.take() {
                    player.seek_to(seek_pos);
                }
            }
        }

        let has_finite_animation = !self.test_animations_paused
            && self
                .runtime
                .runtime_state
                .motion
                .active
                .values()
                .any(|anim| !anim.repeat);
        let resize_unsettled = resize_is_unsettled(
            self.pending_resize.is_some(),
            self.resize_needs_settled_frame,
            self.live_resize.is_live(now),
        );
        let repeat_animation_interval =
            if self.test_animations_paused || resize_unsettled || self.pending_capture_settle {
                None
            } else {
                repeating_animation_redraw_interval(
                    &self.runtime.runtime_state.motion,
                    self.repeat_animation_frame,
                )
            };
        let has_playing_video = self.players.iter().any(|(widget_id, _)| {
            self.runtime
                .runtime_state
                .video
                .states
                .get(widget_id)
                .map(|state| state.status == VideoStatus::Playing)
                .unwrap_or(false)
        });
        let animation_frame = animation_redraw_interval(
            has_finite_animation,
            repeat_animation_interval,
            has_playing_video,
            self.min_frame,
        );

        self.ime_handler
            .set_text_input_config(focused_text_input_config(
                &self.runtime,
                self.pipeline.prev_ir.as_ref(),
            ));
        let focused_text_input =
            focused_text_input_id(&self.runtime, self.pipeline.prev_ir.as_ref());
        if focused_text_input != self.blink_focus_id {
            if let Some(prev) = self.blink_focus_id {
                self.runtime.runtime_state.caret_visible.remove(&prev);
            }
            self.blink_focus_id = focused_text_input;
            if let Some(id) = self.blink_focus_id {
                self.runtime.runtime_state.caret_visible.insert(id, true);
                self.last_blink_toggle = now;
                self.invalidations.mark_build();
                request_redraw_logged(
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "caret_focus_changed",
                );
            }
        }

        // Cursor blink: toggle visibility and request a redraw.
        if self.blink_enabled {
            if let Some(id) = self.blink_focus_id {
                if now.duration_since(self.last_blink_toggle) >= self.blink_period {
                    let visible = self
                        .runtime
                        .runtime_state
                        .caret_visible
                        .get(&id)
                        .copied()
                        .unwrap_or(true);
                    self.runtime
                        .runtime_state
                        .caret_visible
                        .insert(id, !visible);
                    self.last_blink_toggle = now;
                    self.invalidations.mark_build();
                    request_redraw_logged(
                        &window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        "caret_blink",
                    );
                }
            }
        }

        let blink_wake_at = if self.blink_enabled && self.blink_focus_id.is_some() {
            Some(self.last_blink_toggle + self.blink_period)
        } else {
            None
        };

        // Drain completed background effect results and dispatch
        // their continuations back into the self.runtime on the main thread.
        let effect_results_dispatched = drain_effect_results(
            &mut self.runtime,
            &self.effect_result_rx,
            &mut self.active_services,
            &mut self.service_bindings,
        );
        if effect_results_dispatched {
            self.invalidations.mark_build();
            // Background work completed — process any new effects
            // the continuation reducers may have emitted.
            if process_pending_effects(
                &mut self.runtime,
                &self.effect_result_tx,
                &self.event_proxy,
                &self.async_registry,
                &mut self.active_services,
                &mut self.service_bindings,
                &mut self.next_service_instance_id,
            ) {
                self.invalidations.mark_build();
                request_redraw_logged(
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "effect_continuation",
                );
            }
            request_redraw_logged(
                &window,
                elwt,
                &mut self.last_redraw_at,
                self.min_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                "effect_result",
            );
        }

        // Application frame hook (e.g. LSP polling).
        let frame_hook_wants_redraw = if let Some(ref hook) = self.frame_hook {
            let hook = hook.clone();
            if let Some(state) = self.runtime.get_global_state_mut::<S>() {
                hook(state)
            } else {
                false
            }
        } else {
            false
        };
        if frame_hook_wants_redraw {
            self.invalidations.mark_build();
            request_redraw_logged(
                &window,
                elwt,
                &mut self.last_redraw_at,
                self.min_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                "frame_hook",
            );
        }

        let next_frame_resource_generation = self.frame_submission.resource_generation();
        let frame_resources_changed =
            next_frame_resource_generation != self.frame_resource_generation;
        if frame_resources_changed {
            self.frame_resource_generation = next_frame_resource_generation;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.retained_scene_cache.clear();
            }
            #[cfg(target_arch = "wasm32")]
            {
                if let Some(WebRenderer::WebGpu(presenter)) = self.web_renderer.as_mut() {
                    presenter.retained_scene_cache.clear();
                }
            }
            self.invalidations.mark_paint();
        }

        let next_vello_image_generation = fission_render_vello::image_cache_generation();
        #[cfg(target_arch = "wasm32")]
        let next_software_image_generation = fission_render_software::image_cache_generation();
        #[cfg(target_arch = "wasm32")]
        let image_cache_changed = next_vello_image_generation != self.vello_image_cache_generation
            || next_software_image_generation != self.software_image_cache_generation;
        #[cfg(not(target_arch = "wasm32"))]
        let image_cache_changed = next_vello_image_generation != self.vello_image_cache_generation;
        if image_cache_changed {
            self.vello_image_cache_generation = next_vello_image_generation;
            #[cfg(target_arch = "wasm32")]
            {
                self.software_image_cache_generation = next_software_image_generation;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.retained_scene_cache.clear();
            }
            #[cfg(target_arch = "wasm32")]
            {
                if let Some(WebRenderer::WebGpu(presenter)) = self.web_renderer.as_mut() {
                    presenter.retained_scene_cache.clear();
                }
            }
            self.invalidations.mark_paint();
            let stats = fission_render_vello::image_cache_stats();
            diag::emit(
                diag::DiagCategory::Raster,
                diag::DiagLevel::Debug,
                diag::DiagEventKind::ImageCacheSummary {
                    renderer: "vello".to_string(),
                    entries: stats.entries,
                    weighted_bytes: stats.weighted_bytes,
                    max_bytes: stats.max_bytes,
                    pending: stats.pending,
                    hits: stats.hits,
                    misses: stats.misses,
                    loads_started: stats.loads_started,
                    loads_completed: stats.loads_completed,
                    loads_failed: stats.loads_failed,
                    evictions: stats.evictions,
                    offscreen_skips: stats.offscreen_skips,
                },
            );
            #[cfg(target_arch = "wasm32")]
            {
                let stats = fission_render_software::image_cache_stats();
                diag::emit(
                    diag::DiagCategory::Raster,
                    diag::DiagLevel::Debug,
                    diag::DiagEventKind::ImageCacheSummary {
                        renderer: "software".to_string(),
                        entries: stats.entries,
                        weighted_bytes: stats.weighted_bytes,
                        max_bytes: stats.max_bytes,
                        pending: stats.pending,
                        hits: stats.hits,
                        misses: stats.misses,
                        loads_started: stats.loads_started,
                        loads_completed: stats.loads_completed,
                        loads_failed: stats.loads_failed,
                        evictions: stats.evictions,
                        offscreen_skips: 0,
                    },
                );
            }
            request_redraw_logged(
                &window,
                elwt,
                &mut self.last_redraw_at,
                self.min_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                "image_cache",
            );
        }
        #[cfg(target_arch = "wasm32")]
        let image_cache_pending = fission_render_vello::image_cache_has_pending()
            || fission_render_software::image_cache_has_pending();
        #[cfg(not(target_arch = "wasm32"))]
        let image_cache_pending = fission_render_vello::image_cache_has_pending();
        let resource_work_pending =
            image_cache_pending || self.frame_submission.has_pending_resources();

        // When a frame_hook is registered, ensure the event loop
        // wakes at least every 2 seconds so the hook fires even
        // when no user input or animation is happening (e.g. for
        // asynchronous LSP diagnostics).
        let frame_hook_wake_at = if self.frame_hook.is_some() {
            Some(now + Duration::from_secs(2))
        } else {
            None
        };

        let has_pending_work = effect_results_dispatched
            || frame_hook_wants_redraw
            || image_cache_changed
            || frame_resources_changed
            || self.invalidations.any()
            || resize_unsettled
            || self.pending_capture_settle;
        let active_keys = active_animation_keys(&self.runtime);

        if has_pending_work {
            let pending_frame = pending_work_redraw_interval(
                self.invalidations,
                resize_unsettled || self.pending_capture_settle,
                self.min_frame,
                self.resize_frame,
            );
            let redraw_reason = if resize_unsettled {
                "self.pending_resize"
            } else if self.pending_capture_settle {
                "self.pending_capture_settle"
            } else if self.invalidations.build {
                "pending_work:build"
            } else if self.invalidations.layout {
                "pending_work:layout"
            } else if self.invalidations.paint {
                "pending_work:paint"
            } else if self.invalidations.composite {
                "pending_work:composite"
            } else if effect_results_dispatched {
                "pending_work:effects"
            } else if frame_hook_wants_redraw {
                "pending_work:frame_hook"
            } else {
                "pending_work"
            };
            request_redraw_logged(
                &window,
                elwt,
                &mut self.last_redraw_at,
                pending_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                redraw_reason,
            );
            let reasons = self.frame_trace.take_redraw_reasons();
            self.frame_trace.emit(
                    "about_to_wait",
                    self.presented_frames + 1,
                    &active_keys,
                    self.invalidations,
                    &reasons,
                    &format!(
                        "schedule=pending interval_ms={} self.pending_resize={} self.redraw_pending={} highest={}",
                        pending_frame.as_millis(),
                        resize_unsettled || self.pending_capture_settle,
                        self.redraw_pending,
                        self.invalidations.highest_class(),
                    ),
                );
            let mut wake_at = self.last_redraw_at + pending_frame;
            if let Some(blink_at) = blink_wake_at {
                if blink_at < wake_at {
                    wake_at = blink_at;
                }
            }
            if let Some(hook_at) = frame_hook_wake_at {
                if hook_at < wake_at {
                    wake_at = hook_at;
                }
            }
            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
        } else if let Some(animation_frame) = animation_frame {
            request_redraw_logged(
                &window,
                elwt,
                &mut self.last_redraw_at,
                animation_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                if has_finite_animation {
                    "animation:finite"
                } else if has_playing_video {
                    "animation:video"
                } else {
                    "animation:repeat"
                },
            );
            let reasons = self.frame_trace.take_redraw_reasons();
            self.frame_trace.emit(
                    "about_to_wait",
                    self.presented_frames + 1,
                    &active_keys,
                    self.invalidations,
                    &reasons,
                    &format!(
                        "schedule=animation interval_ms={} self.pending_resize={} self.redraw_pending={} highest={}",
                        animation_frame.as_millis(),
                        resize_unsettled || self.pending_capture_settle,
                        self.redraw_pending,
                        self.invalidations.highest_class(),
                    ),
                );
            let mut wake_at = self.last_redraw_at + animation_frame;
            if let Some(blink_at) = blink_wake_at {
                if blink_at < wake_at {
                    wake_at = blink_at;
                }
            }
            if let Some(hook_at) = frame_hook_wake_at {
                if hook_at < wake_at {
                    wake_at = hook_at;
                }
            }
            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
        } else if resource_work_pending {
            let wake_at = now + Duration::from_millis(50);
            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
        } else if let Some(blink_at) = blink_wake_at {
            let reasons = self.frame_trace.take_redraw_reasons();
            self.frame_trace.emit(
                    "about_to_wait",
                    self.presented_frames + 1,
                    &active_keys,
                    self.invalidations,
                    &reasons,
                    "schedule=blink_wait self.pending_resize=false self.redraw_pending=false highest=none",
                );
            let mut wake_at = blink_at;
            if let Some(hook_at) = frame_hook_wake_at {
                if hook_at < wake_at {
                    wake_at = hook_at;
                }
            }
            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
        } else if let Some(hook_at) = frame_hook_wake_at {
            let reasons = self.frame_trace.take_redraw_reasons();
            self.frame_trace.emit(
                    "about_to_wait",
                    self.presented_frames + 1,
                    &active_keys,
                    self.invalidations,
                    &reasons,
                    "schedule=hook_wait self.pending_resize=false self.redraw_pending=false highest=none",
                );
            elwt.set_control_flow(ControlFlow::WaitUntil(hook_at));
        } else {
            let reasons = self.frame_trace.take_redraw_reasons();
            self.frame_trace.emit(
                "about_to_wait",
                self.presented_frames + 1,
                &active_keys,
                self.invalidations,
                &reasons,
                "schedule=idle self.pending_resize=false self.redraw_pending=false highest=none",
            );
            #[cfg(target_os = "android")]
            if self.test_control_enabled {
                elwt.set_control_flow(ControlFlow::Poll);
            } else {
                elwt.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(16)));
            }
            #[cfg(not(target_os = "android"))]
            elwt.set_control_flow(ControlFlow::Wait);
        }
    }
}
