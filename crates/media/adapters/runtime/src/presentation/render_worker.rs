//! Composites one shared state snapshot across the hosted outputs.

use super::*;

impl HostedOutput {
    fn update_standby(&mut self, reason: Option<crate::standby::Reason>, endpoint: &str) {
        if reason == self.standby_reason {
            return;
        }
        self.standby_reason = reason;
        self.standby = reason.and_then(|reason| {
            crate::standby::render(self.output.size(), endpoint, reason)
                .and_then(|frame| {
                    SourceTexture::from_rgba8(self.output.gpu(), frame.size, &frame.pixels)
                        .map_err(anyhow::Error::from)
                })
                .map_err(|error| tracing::error!(%error, "cannot update the Media standby surface"))
                .ok()
        });
    }
}

impl RenderWorkerState {
    pub(super) fn now(&self) -> Timestamp {
        Timestamp::from_micros(self.started.elapsed().as_micros() as u64)
    }

    pub(super) fn present_all(&mut self) {
        let now = self.now();
        let seconds = self.started.elapsed().as_secs_f32();
        let state = self.state.load();
        let catalog = self.catalog.load();
        // Every output in this pass composites from the same configuration snapshot.
        let configuration = self.configuration.load();
        // Without an input device, time-driven visualizers run while audio-driven ones rest.
        let heard = self.analysis.load();
        let mut reports = Vec::new();

        for hosted in &mut self.outputs {
            crate::pixel_output::follow_pixel_map(&mut hosted.configuration, &configuration);
            if !hosted.output.should_present(now) {
                continue;
            }
            let Some(output_state) = state.output(hosted.output.id()) else {
                continue;
            };
            let resolved_output = crate::effect_banks::resolve_output(output_state, &configuration);
            let output_state = &resolved_output;
            hosted.sync_models(&self.models, &configuration, output_state);
            let master = output_state.master;
            let region = shown_region(&configuration, output_state.id);
            let status_overlay = configuration
                .output(output_state.id)
                .is_some_and(|output| output.status_overlay);
            let standby_reason = crate::standby::reason(
                status_overlay,
                output_state.ownership.dmx_is_active(now),
                output_state.ownership.web_takeover,
                catalog.item_count() == 0,
                self.configuration_issue,
            );
            hosted.update_standby(standby_reason, &self.administration_endpoint);
            if present_standby(
                &mut self.sinks,
                &self.test_pattern_layer,
                &self.operator_overlay_layer,
                hosted,
                output_state,
                now,
                region,
            ) {
                continue;
            }

            if present_direct(
                &mut self.loader,
                self.direct.as_mut(),
                &mut self.sinks,
                &self.operator_overlay_layer,
                hosted,
                output_state,
                &master,
                now,
                region,
            ) {
                continue;
            }

            // The real path: every layer's address becomes a texture, or reports why it did not.
            let prepared = hosted.pipeline.prepare(
                output_state,
                crate::layer_pipeline::FrameContext::heard(
                    &catalog,
                    &configuration,
                    &heard,
                    unix_millis(),
                    seconds,
                    now,
                )
                .with_tempo(tempo_of(
                    &configuration,
                    output_state.id,
                    &self.speed_groups,
                )),
                &mut self.loader,
            );
            reports.extend(
                prepared
                    .statuses
                    .iter()
                    .map(|(layer, status)| (output_state.id, *layer, *status)),
            );

            let effective_layers =
                hosted.apply_layer_effects(output_state, &prepared, seconds, &heard);
            let mut draws = hosted
                .pipeline
                .draws_from_layers(&effective_layers, &prepared);
            // The diagnostic pattern occupies layer one only while nothing else has been
            // selected, so it can never hide a running show.
            if draws.is_empty()
                && let Some(pattern) = hosted.test_pattern.as_ref()
            {
                draws.push(LayerDraw {
                    state: &self.test_pattern_layer,
                    source: pattern,
                    mask: None,
                });
            }

            let master_mask = prepared
                .master_mask
                .and_then(|slot| hosted.pipeline.texture(slot));
            let overlay = operator_overlay(
                hosted.hint_visible_until,
                hosted.fullscreen_hint.as_ref(),
                &self.operator_overlay_layer,
            );
            present(
                &mut hosted.output,
                &draws,
                &master,
                master_mask,
                now,
                region,
                overlay,
            );
            capture_previews(
                &mut self.sinks,
                &hosted.configuration,
                &mut hosted.output,
                output_state,
                &effective_layers,
                &draws,
                &master,
                master_mask,
                now,
            );
        }

        self.publish(reports, now);
    }

    /// Tells the reducer what each layer's source did, so the API, the UI, and CITP all report the
    /// lifecycle the renderer actually saw rather than each guessing at it.
    fn publish(
        &self,
        reports: Vec<(media_domain::OutputId, usize, media_domain::SourceStatus)>,
        now: Timestamp,
    ) {
        if reports.is_empty() {
            return;
        }
        self.state.rcu(|current| {
            with_reports(current, &reports, now)
                .map(Arc::new)
                .unwrap_or_else(|| Arc::clone(current))
        });
    }
}
