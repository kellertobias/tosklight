//! Drawing one frame: what is uploaded, in what order the passes run, and what each of them is.
//!
//! The renderer itself holds the resources; this is what it does with them. Every pass is its own
//! method so the order of a frame can be read at a glance in [`Renderer::render`] — cull, shadows,
//! opaque, beams, lasers, bloom, composite, overlay — with the detail of any one of them a step
//! away rather than inline.

use super::{
    BLOOM_MIX, FrameStats, Globals, MAX_SHADOWS, RenderError, Renderer, SHADOW_DRAW_STRIDE,
    SHADOW_TILE_EDGE, SHADOW_TILES_PER_ROW,
};
use crate::camera::ResolvedCamera;
use crate::instances::{FrameStyle, GpuLight, MeshKind};
use bytemuck::Zeroable;
use glam::{Mat4, Vec3};
use viz_scene::{Aabb, Scene, SceneValues, ViewConfiguration};

/// What one frame decided before any of it was drawn.
struct FramePlan {
    plot: bool,
    draw_beams: bool,
    passes: PassPlan,
    volumetric_steps: u32,
    shadow_budget: u32,
    render_scale: f32,
    adaptive_degraded: bool,
    exposure: f32,
    camera: ResolvedCamera,
}

/// Expensive light-simulation passes selected for a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PassPlan {
    cull: bool,
    shadows: bool,
    bloom: bool,
}

impl PassPlan {
    fn for_view(view: &ViewConfiguration) -> Self {
        let light = view.mode.simulates_light();
        Self {
            cull: light,
            shadows: light && view.quality.shadow_budget() > 0,
            bloom: light && view.quality.bloom_enabled(),
        }
    }
}

impl Renderer {
    pub fn render(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        overlay: &crate::overlay::Overlay,
        time_seconds: f32,
    ) -> Result<FrameStats, RenderError> {
        let started = std::time::Instant::now();
        // Progress asynchronous timestamp mappings without ever waiting for the GPU. Without this
        // poll, native backends are allowed to leave a completed map callback queued forever.
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
        if let Some(timer) = self.timer.as_mut() {
            timer.collect();
        }
        if let Some((sample_id, gpu_micros)) = self.timer.as_ref().and_then(|timer| {
            timer
                .timings()
                .total_micros()
                .map(|micros| (timer.sample_id(), micros))
        }) && sample_id != self.last_timing_sample
        {
            self.last_timing_sample = sample_id;
            if view.quality == viz_scene::RenderQuality::Ultra {
                self.ultra_budget.observe(gpu_micros);
            }
        }
        let plan = self.plan_frame(scene, values, view);
        let device = self.gpu.device.clone();
        let queue = self.gpu.queue.clone();

        self.assign_shadows(view, plan.shadow_budget);
        self.upload_frame(scene, &device, &queue);
        self.write_globals(&plan, values, view, time_seconds, &queue);

        let capture = self.capture_request.take();
        let before_acquire = std::time::Instant::now();
        let mut acquire_micros = 0;
        let (frame, output) = match &capture {
            Some(view) => (None, view.clone()),
            None => {
                let frame = self.acquire()?;
                acquire_micros = before_acquire.elapsed().as_micros() as u64;
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                (Some(frame), view)
            }
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viz frame"),
        });

        if plan.passes.cull {
            self.cull_pass(&mut encoder);
        }
        if plan.passes.shadows {
            self.shadow_pass(&mut encoder);
        }

        // Multisampled colour is resolved by whichever shaded pass writes last, so the samples
        // are never resolved twice and the beams are anti-aliased with the geometry rather than
        // composited on top of an already-flattened image.
        let beams_drawn =
            plan.draw_beams && self.beam_instances.length > 0 && values.atmosphere.density > 0.0005;
        let lasers_drawn = plan.draw_beams && self.laser_instances.length > 0;
        let mut draw_calls = 0_u32;
        let mut instance_total = 0_u32;
        self.opaque_pass(
            &mut encoder,
            view,
            beams_drawn,
            &mut draw_calls,
            &mut instance_total,
        );
        if beams_drawn {
            self.beam_pass(&mut encoder);
        }
        if lasers_drawn {
            self.laser_pass(&mut encoder);
        }
        self.post_passes(&mut encoder, &queue, &plan, view, &output);
        if !overlay.is_empty() {
            self.overlay_pass(&mut encoder, &device, &queue, overlay, &output);
        }

        if let Some(timer) = self.timer.as_mut() {
            timer.resolve(&mut encoder);
        }
        queue.submit(Some(encoder.finish()));
        if let Some(timer) = self.timer.as_ref() {
            timer.request_readback();
        }
        if let Some(frame) = frame {
            queue.present(frame);
        }

        let gpu_passes = self
            .timer
            .as_ref()
            .map(crate::timing::GpuTimer::timings)
            .unwrap_or_default();
        self.stats = FrameStats {
            cpu_micros: started.elapsed().as_micros() as u64,
            acquire_micros,
            lights: self.lights.length,
            beams: self.beam_instances.length,
            instances: instance_total,
            particles_requested: self.frame.particles_requested,
            particles_drawn: self.frame.particles_drawn,
            draw_calls,
            // Only report degradation the renderer actually applied this frame.
            degraded: self.frame.lights.len() as u32 > self.lights.length
                || self.beam_overflow
                || plan.adaptive_degraded
                || self.frame.crowd_drawn < self.frame.crowd_requested
                || self.frame.particles_drawn < self.frame.particles_requested,
            gpu_micros: gpu_passes.total_micros(),
            gpu_passes,
            volumetric_steps: plan.volumetric_steps,
            shadow_budget: plan.shadow_budget,
            render_scale: plan.render_scale,
            crowd_authored: self.frame.crowd_authored,
            crowd_drawn: self.frame.crowd_drawn,
        };
        Ok(self.stats)
    }

    /// Everything about this frame that is decided before anything is uploaded or drawn.
    fn plan_frame(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
    ) -> FramePlan {
        let plot = view.mode.is_plot();
        let (volumetric_steps, shadow_budget, adaptive_scale, adaptive_degraded) =
            if view.quality == viz_scene::RenderQuality::Ultra {
                let (steps, shadows, scale) = self.ultra_budget.settings();
                (steps, shadows, scale, self.ultra_budget.degraded())
            } else {
                (
                    view.quality.volumetric_steps(),
                    view.quality.shadow_budget(),
                    1.0,
                    false,
                )
            };
        // A plan is drawn at the display's own resolution whatever the tier says: a stage plot is
        // outlines and type, and a cheaper tier must not soften the lines an operator prints.
        self.set_target_scale(if plot {
            1.0
        } else {
            view.quality.resolution_scale() * adaptive_scale
        });
        let control = crate::camera::CameraControl::from_camera(&view.camera);
        let (plot_right, plot_up) = control.page_axes(view.mode);
        let style = FrameStyle {
            quality: view.quality,
            draw_beams: view.mode.renders_beams(),
            draw_aim_lines: view.mode.renders_aim_lines(),
            plot,
            plot_right,
            plot_up,
            projection_view: view
                .mode
                .projection_view()
                .unwrap_or(viz_scene::ProjectionView::Top),
            symbol_metres: symbol_metres(view),
            beam_ink: Vec3::from(view.theme.beam_ink()),
            ink: Vec3::from(view.theme.ink()),
            faint_ink: Vec3::from(view.theme.faint_ink()),
            symbol_ink: Vec3::from(view.theme.symbol_ink()),
            selected_ink: Vec3::from(view.theme.selected_ink()),
            fixture_models: view.mode.draws_fixture_models(),
            emitter_apertures: view.mode.simulates_light(),
            scenery_surfaces: view.mode.simulates_light(),
            aim_guides: view.mode.always_draws_aim_guides(),
            // The grid is a ground reference for a picture with a ground in it. A plan already
            // has one — it is a drawing on a page with its own scale — so it is not drawn there.
            floor_grid: view.floor_grid && !plot,
            scenery: match view.mode {
                viz_scene::ViewMode::Lines3d => {
                    |kind| viz_scene::ViewMode::Lines3d.draws_scenery(kind)
                }
                _ => |_| true,
            },
            quality: view.quality,
            crowd_amount: self.crowd_amount,
            crowd_person_budget: match view.quality {
                viz_scene::RenderQuality::Draft | viz_scene::RenderQuality::Standard => 0,
                viz_scene::RenderQuality::High => 384,
                viz_scene::RenderQuality::Ultra => {
                    (1_024.0 * adaptive_scale * adaptive_scale).round() as usize
                }
            },
            effect_particle_budget: match view.quality {
                viz_scene::RenderQuality::Draft => 128,
                viz_scene::RenderQuality::Standard => 512,
                viz_scene::RenderQuality::High => 2_048,
                viz_scene::RenderQuality::Ultra => {
                    (8_192.0 * adaptive_scale * adaptive_scale).round() as usize
                }
            },
        };
        let draw_beams = style.draw_beams && !plot;
        self.frame = crate::instances::build(scene, values, &style);

        // One exposure, the operator's trim over it, and nothing that watches the rig and moves.
        let exposure = (super::BASE_EXPOSURE * view.exposure).clamp(0.02, 4.0);
        let bounds = if scene.bounds.is_empty() {
            Aabb {
                min: Vec3::new(-6.0, 0.0, -6.0),
                max: Vec3::new(6.0, 8.0, 6.0),
            }
        } else {
            scene.bounds
        };
        let camera = ResolvedCamera::resolve(&view.camera, view.mode, self.gpu.aspect(), bounds);
        FramePlan {
            plot,
            draw_beams,
            passes: PassPlan::for_view(view),
            volumetric_steps,
            shadow_budget,
            render_scale: if plot {
                1.0
            } else {
                view.quality.resolution_scale() * adaptive_scale
            },
            adaptive_degraded,
            exposure,
            camera,
        }
    }

    /// Pick the shadow casters and stamp each chosen light with its tile before the light buffer
    /// is uploaded, so the shaders read the same frame the maps were drawn for.
    fn assign_shadows(&mut self, view: &ViewConfiguration, quality_budget: u32) {
        let budget = if view.mode.renders_beams() {
            quality_budget as usize
        } else {
            0
        };
        let chosen = shadow_candidates(&self.frame.lights, budget);
        let tile = SHADOW_TILE_EDGE as f32 / super::SHADOW_ATLAS_EDGE as f32;
        let mut matrices: Vec<[[f32; 4]; 4]> = Vec::with_capacity(chosen.len());
        for (slot, light_index) in chosen.iter().enumerate() {
            let matrix = shadow_matrix(&self.frame.lights[*light_index]);
            matrices.push(matrix.to_cols_array_2d());
            let column = slot as u32 % SHADOW_TILES_PER_ROW;
            let row = slot as u32 / SHADOW_TILES_PER_ROW;
            self.frame.lights[*light_index].shadow =
                [slot as f32, column as f32 * tile, row as f32 * tile, tile];
        }
        self.shadow_count = matrices.len() as u32;
        if matrices.is_empty() {
            // The bind group still needs something to point at.
            matrices.push(Mat4::IDENTITY.to_cols_array_2d());
        }
        let mut draws = vec![0_u8; matrices.len() * SHADOW_DRAW_STRIDE as usize];
        for slot in 0..matrices.len() {
            let offset = slot * SHADOW_DRAW_STRIDE as usize;
            draws[offset..offset + 4].copy_from_slice(&(slot as u32).to_le_bytes());
        }
        if self
            .shadow_matrices
            .upload(&self.gpu.device.clone(), &self.gpu.queue.clone(), &matrices)
        {
            self.rebuild_shadow_groups();
        }
        self.gpu.queue.write_buffer(&self.shadow_draws, 0, &draws);
    }

    /// Everything this frame draws, uploaded to the buffers the passes read.
    fn upload_frame(&mut self, scene: &Scene, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut invalidated = false;
        if self.frame.lights.is_empty() {
            invalidated |= self.lights.upload(device, queue, &[GpuLight::zeroed()]);
            self.lights.length = 0;
        } else {
            invalidated |= self.lights.upload(device, queue, &self.frame.lights);
        }
        if invalidated {
            self.rebuild_scene_groups();
        }
        // A fixture model becomes GPU geometry the first frame it is drawn, and stays until the
        // scene changes. Building it here keeps model loading out of the per-frame path.
        self.ensure_model_meshes(scene);
        self.ensure_gobo_atlas(scene);
        for (kind, instances) in &self.frame.meshes {
            if let Some(buffer) = self.mesh_instances.get_mut(kind) {
                buffer.upload(device, queue, instances);
            }
        }
        let known: Vec<MeshKind> = self.mesh_instances.keys().copied().collect();
        for kind in known {
            if !self
                .frame
                .meshes
                .iter()
                .any(|(existing, _)| *existing == kind)
                && let Some(buffer) = self.mesh_instances.get_mut(&kind)
            {
                buffer.length = 0;
            }
        }
        self.beam_instances.upload(device, queue, &self.frame.beams);
        self.laser_instances
            .upload(device, queue, &self.frame.lasers);
        self.line_vertices.upload(device, queue, &self.frame.lines);
    }

    /// The one uniform block every shader reads: where the camera is, and what the look is.
    fn write_globals(
        &self,
        plan: &FramePlan,
        values: &SceneValues,
        view: &ViewConfiguration,
        time_seconds: f32,
        queue: &wgpu::Queue,
    ) {
        let (camera, exposure) = (&plan.camera, plan.exposure);
        let globals = Globals {
            view_projection: camera.view_projection.to_cols_array_2d(),
            view: camera.view.to_cols_array_2d(),
            inverse_projection: camera.projection.inverse().to_cols_array_2d(),
            camera_position: camera.position.extend(time_seconds).to_array(),
            screen: [
                self.targets.width as f32,
                self.targets.height as f32,
                1.0 / self.targets.width as f32,
                1.0 / self.targets.height as f32,
            ],
            params: [exposure, values.atmosphere.density, camera.near, camera.far],
            params2: [
                self.lights.length as f32,
                plan.volumetric_steps as f32,
                // The ambient level is what the operator sees on screen, so it is divided back
                // out of the adaptation: a rig full of beams pulls the exposure down and would
                // otherwise take the trusses with it.
                (view.ambient.clamp(0.0, 1.0) / exposure.max(0.02)).min(4.0),
                self.targets.tiles_x as f32,
            ],
            params3: [
                f32::from(u8::from(plan.plot)),
                view.quality.fog_detail(),
                time_seconds,
                view.laser_brightness.clamp(0.0, 4.0),
            ],
            params4: [
                f32::from(u8::from(view.quality.draws_gobos())),
                f32::from(u8::from(view.quality.draws_beam_falloff())),
                f32::from(u8::from(plan.plot || !view.mode.simulates_light())),
                0.0,
            ],
        };
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
    }

    /// Sort the rig's lights into screen tiles, so a fragment tests the handful over it rather
    /// than every light in the show.
    fn cull_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.lights.length > 0 {
            let timing = self
                .timer
                .as_ref()
                .and_then(|timer| timer.compute_writes(crate::timing::GpuPass::Cull));
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("viz light cull"),
                timestamp_writes: timing,
            });
            pass.set_pipeline(&self.cull_pipeline);
            pass.set_bind_group(0, &self.cull_bind_group, &[]);
            pass.dispatch_workgroups(self.targets.tiles_x, self.targets.tiles_y, 1);
        } else {
            encoder.clear_buffer(&self.tile_counts.buffer, 0, None);
        }
    }

    /// Shadow maps, drawn before anything reads them. Every map is a tile of one atlas, so the
    /// whole budget costs one attachment and one clear.
    fn shadow_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.shadow_count == 0 {
            return;
        }
        let timing = self
            .timer
            .as_ref()
            .and_then(|timer| timer.render_writes(crate::timing::GpuPass::Shadow));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viz shadows"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_atlas,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.shadow_pipeline);
        for slot in 0..self.shadow_count {
            let tile_x = (slot % SHADOW_TILES_PER_ROW) * SHADOW_TILE_EDGE;
            let tile_y = (slot / SHADOW_TILES_PER_ROW) * SHADOW_TILE_EDGE;
            pass.set_viewport(
                tile_x as f32,
                tile_y as f32,
                SHADOW_TILE_EDGE as f32,
                SHADOW_TILE_EDGE as f32,
                0.0,
                1.0,
            );
            pass.set_bind_group(
                0,
                &self.shadow_draw_group,
                &[slot * SHADOW_DRAW_STRIDE as u32],
            );
            for kind in self.drawn_meshes() {
                let (Some(mesh), Some(instances)) =
                    (self.meshes.get(&kind), self.mesh_instances.get(&kind))
                else {
                    continue;
                };
                if instances.length == 0 {
                    continue;
                }
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_vertex_buffer(1, instances.buffer.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..instances.length);
            }
        }
    }

    /// The picture itself: every body, truss and drape, and the plan's ink lines over them.
    fn opaque_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &ViewConfiguration,
        beams_drawn: bool,
        draw_calls: &mut u32,
        instance_total: &mut u32,
    ) {
        let resolve_in_opaque = (!beams_drawn)
            .then(|| self.targets.resolve_target())
            .flatten();
        let timing = self
            .timer
            .as_ref()
            .and_then(|timer| timer.render_writes(crate::timing::GpuPass::Opaque));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viz opaque"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.hdr,
                depth_slice: None,
                resolve_target: resolve_in_opaque,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background_of(view)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.surface_pipeline);
        pass.set_bind_group(0, &self.scene_bind_group, &[]);
        pass.set_bind_group(2, &self.shadow_bind_group, &[]);
        for kind in self.drawn_meshes() {
            let (Some(mesh), Some(instances)) =
                (self.meshes.get(&kind), self.mesh_instances.get(&kind))
            else {
                continue;
            };
            if instances.length == 0 {
                continue;
            }
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            pass.set_vertex_buffer(1, instances.buffer.slice(..));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..instances.length);
            *draw_calls += 1;
            *instance_total += instances.length;
        }
        if self.line_vertices.length > 0 {
            pass.set_pipeline(&self.line_pipeline);
            pass.set_bind_group(0, &self.scene_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_vertex_buffer(0, self.line_vertices.buffer.slice(..));
            pass.draw(0..self.line_vertices.length, 0..1);
            *draw_calls += 1;
        }
    }

    /// The volumetric shafts, added into the picture the geometry left behind.
    fn beam_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let timing = self
            .timer
            .as_ref()
            .and_then(|timer| timer.render_writes(crate::timing::GpuPass::Beams));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viz beams"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.hdr,
                depth_slice: None,
                resolve_target: self.targets.resolve_target(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.beam_pipeline);
        pass.set_bind_group(0, &self.scene_bind_group, &[]);
        pass.set_bind_group(1, &self.depth_bind_group, &[]);
        pass.set_bind_group(2, &self.shadow_bind_group, &[]);
        pass.set_vertex_buffer(0, self.cone.vertices.slice(..));
        pass.set_vertex_buffer(1, self.beam_instances.buffer.slice(..));
        pass.set_index_buffer(self.cone.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.cone.index_count, 0, 0..self.beam_instances.length);
    }

    /// Lasers go in after the beams and into the same target. They are not gated on haze the
    /// way a beam volume is: a laser path is visible in clear air too, just dimmer, because
    /// the beam is bright enough that even the little scattering an empty room provides shows
    /// it. That is why a laser show can run in a venue where a lantern's shaft is invisible.
    fn laser_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let timing = self
            .timer
            .as_ref()
            .and_then(|timer| timer.render_writes(crate::timing::GpuPass::Lasers));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viz lasers"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.hdr,
                depth_slice: None,
                resolve_target: self.targets.resolve_target(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.laser_pipeline);
        pass.set_bind_group(0, &self.scene_bind_group, &[]);
        pass.set_bind_group(1, &self.depth_bind_group, &[]);
        pass.set_vertex_buffer(0, self.laser_instances.buffer.slice(..));
        pass.draw(0..6, 0..self.laser_instances.length);
    }

    /// Bloom and the composite that puts the frame on screen.
    ///
    /// A picture with no light in it gets no camera put in front of it.
    ///
    /// A plan is ink on paper, and an outline view is a diagram drawn in the same spirit: their
    /// colours are already the final ones. Rolling them through an exposure and a filmic curve
    /// greys the page, and blooming them turns every aim line into a lit beam — which is the one
    /// thing an outline view exists not to claim.
    fn post_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        plan: &FramePlan,
        view: &ViewConfiguration,
        output: &wgpu::TextureView,
    ) {
        let exposure = plan.exposure;
        let drawn = plan.plot || !view.mode.simulates_light();
        let bloom = plan.passes.bloom;
        let composite_exposure = if drawn { 1.0 } else { exposure };
        if bloom {
            queue.write_buffer(
                &self.post_settings,
                0,
                bytemuck::cast_slice(&[exposure, 1.0_f32, 1.0, 0.0]),
            );
            let bloom_opening = self
                .timer
                .as_ref()
                .and_then(|timer| timer.render_opening(crate::timing::GpuPass::Bloom));
            fullscreen_pass_timed(
                encoder,
                "viz bloom extract",
                &self.extract_pipeline,
                &[&self.bloom_extract_group],
                &self.targets.bloom_a,
                bloom_opening,
            );
            fullscreen_pass(
                encoder,
                "viz bloom blur h",
                &self.blur_pipeline,
                &[&self.bloom_blur_a_group],
                &self.targets.bloom_b,
            );
            queue.write_buffer(
                &self.post_settings,
                0,
                bytemuck::cast_slice(&[exposure, 1.0_f32, 0.0, 1.0]),
            );
            let bloom_closing = self
                .timer
                .as_ref()
                .and_then(|timer| timer.render_closing(crate::timing::GpuPass::Bloom));
            fullscreen_pass_timed(
                encoder,
                "viz bloom blur v",
                &self.blur_pipeline,
                &[&self.bloom_blur_b_group],
                &self.targets.bloom_a,
                bloom_closing,
            );
        }
        queue.write_buffer(
            &self.post_settings,
            0,
            bytemuck::cast_slice(&[
                composite_exposure,
                if bloom { BLOOM_MIX } else { 0.0 },
                if drawn { 0.0_f32 } else { 1.0 },
                0.0,
            ]),
        );
        let composite_timing = self
            .timer
            .as_ref()
            .and_then(|timer| timer.render_writes(crate::timing::GpuPass::Composite));
        fullscreen_pass_timed(
            encoder,
            "viz composite",
            &self.composite_pipeline,
            &[&self.composite_source_group, &self.composite_bloom_group],
            output,
            composite_timing,
        );
    }

    /// The status line, Quick Settings and plan labels, drawn over the finished picture.
    fn overlay_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        overlay: &crate::overlay::Overlay,
        output: &wgpu::TextureView,
    ) {
        self.overlay_quads.upload(device, queue, &overlay.quads);
        queue.write_buffer(
            &self.overlay_globals,
            0,
            bytemuck::cast_slice(&[
                self.gpu.config.width as f32,
                self.gpu.config.height as f32,
                1.0 / self.gpu.config.width as f32,
                1.0 / self.gpu.config.height as f32,
            ]),
        );
        let timing = self
            .timer
            .as_ref()
            .and_then(|timer| timer.render_writes(crate::timing::GpuPass::Overlay));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viz overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.overlay_pipeline);
        pass.set_bind_group(0, &self.overlay_bind_group, &[]);
        pass.set_vertex_buffer(0, self.overlay_quads.buffer.slice(..));
        pass.draw(0..6, 0..self.overlay_quads.length);
        drop(pass);
    }
}

/// The page colour behind everything, from the operator's chosen theme.
/// World size of one plan symbol, chosen so it keeps a constant share of the screen height
/// whichever plan direction and zoom the operator is on.
fn symbol_metres(view: &ViewConfiguration) -> f32 {
    const SCREEN_SHARE: f32 = 0.008;
    let half_height = if view.mode.is_orthographic() {
        view.camera.orthographic_size
    } else {
        let distance = (view.camera.position - view.camera.target)
            .length()
            .max(0.5);
        distance * (view.camera.fov_degrees.to_radians() * 0.5).tan()
    };
    (half_height * SCREEN_SHARE * 2.0).clamp(0.03, 1.2)
}

/// Which lights get a shadow map this frame, brightest first.
///
/// A frame has more lights than maps, and the operator notices the shadow of the brightest beam
/// long before the twentieth. Choosing by radiance keeps the budget spent where it shows.
fn shadow_candidates(lights: &[GpuLight], budget: usize) -> Vec<usize> {
    let mut ranked: Vec<(usize, f32)> = lights
        .iter()
        .enumerate()
        .filter(|(_, light)| light.colour_intensity[3] > 0.02)
        .map(|(index, light)| {
            let radiance = light.colour_intensity[0]
                .max(light.colour_intensity[1])
                .max(light.colour_intensity[2]);
            (index, radiance)
        })
        .collect();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    ranked.truncate(budget.min(MAX_SHADOWS));
    ranked.into_iter().map(|(index, _)| index).collect()
}

/// The view-projection one light renders its depth map with.
fn shadow_matrix(light: &GpuLight) -> Mat4 {
    let position = Vec3::from_slice(&light.position_range[..3]);
    let range = light.position_range[3].max(1.0);
    let direction = Vec3::from_slice(&light.direction_cos_outer[..3]).normalize_or(Vec3::NEG_Y);
    let cos_outer = light.direction_cos_outer[3].clamp(-0.999, 0.999);
    // A little wider than the cone so the edge of the field is inside the map.
    let half_angle = (cos_outer.acos() * 1.25).clamp(0.05, 1.4);
    let up = if direction.y.abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = Mat4::look_at_rh(position, position + direction, up);
    let projection = Mat4::perspective_rh(half_angle * 2.0, 1.0, 0.15, range.max(4.0));
    projection * view
}

fn background_of(view: &ViewConfiguration) -> wgpu::Color {
    let [red, green, blue] = view.background_colour();
    wgpu::Color {
        r: f64::from(red),
        g: f64::from(green),
        b: f64::from(blue),
        a: 1.0,
    }
}

fn fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    pipeline: &wgpu::RenderPipeline,
    groups: &[&wgpu::BindGroup],
    target: &wgpu::TextureView,
) {
    fullscreen_pass_timed(encoder, label, pipeline, groups, target, None);
}

fn fullscreen_pass_timed(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    pipeline: &wgpu::RenderPipeline,
    groups: &[&wgpu::BindGroup],
    target: &wgpu::TextureView,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    for (index, group) in groups.iter().enumerate() {
        pass.set_bind_group(index as u32, *group, &[]);
    }
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_light_views_issue_no_cull_shadow_or_bloom_work() {
        for mode in viz_scene::ViewMode::ALL {
            let view = ViewConfiguration {
                mode,
                quality: viz_scene::RenderQuality::Ultra,
                ..ViewConfiguration::default()
            };
            let passes = PassPlan::for_view(&view);
            if mode.simulates_light() {
                assert!(passes.cull, "{mode:?}");
            } else {
                assert_eq!(
                    passes,
                    PassPlan {
                        cull: false,
                        shadows: false,
                        bloom: false,
                    },
                    "{mode:?}"
                );
            }
        }
    }

    fn light(radiance: f32) -> GpuLight {
        GpuLight {
            position_range: [0.0, 6.0, 0.0, 20.0],
            direction_cos_outer: [0.0, -1.0, 0.0, 0.9],
            colour_intensity: [radiance, radiance, radiance, radiance],
            params: [0.95, 0.2, 0.0, 0.05],
            tangent_frost: [1.0, 0.0, 0.0, 0.0],
            optics: [0.0; 4],
            shapers: [0.0; 4],
            shaper_angles: [0.0; 4],
            gate: [-1.0, 0.0, 0.0, 0.0],
            shadow: [-1.0, 0.0, 0.0, 0.0],
        }
    }

    /// There are always more lights than maps. The budget has to go to the beams an operator is
    /// actually looking at, which are the bright ones.
    #[test]
    fn the_shadow_budget_goes_to_the_brightest_beams() {
        let lights = vec![light(0.1), light(0.9), light(0.5), light(0.0)];
        let chosen = shadow_candidates(&lights, 2);
        assert_eq!(chosen, vec![1, 2]);
    }

    #[test]
    fn a_dark_light_never_costs_a_shadow_map() {
        let lights = vec![light(0.0), light(0.0)];
        assert!(shadow_candidates(&lights, 4).is_empty());
    }

    #[test]
    fn the_budget_can_never_exceed_the_atlas() {
        let lights: Vec<GpuLight> = (0..64).map(|index| light(index as f32 + 1.0)).collect();
        assert_eq!(shadow_candidates(&lights, 1000).len(), MAX_SHADOWS);
    }

    /// A point in front of the lamp has to land inside the map, or the light is shadowing itself
    /// against nothing.
    #[test]
    fn a_point_in_the_beam_projects_inside_its_own_shadow_map() {
        let light = light(1.0);
        let matrix = shadow_matrix(&light);
        let point = Vec3::new(0.0, 1.0, 0.0);
        let clip = matrix * point.extend(1.0);
        assert!(clip.w > 0.0, "the point is in front of the lamp");
        let ndc = clip.truncate() / clip.w;
        assert!(
            ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0,
            "inside the map: {ndc:?}"
        );
        assert!(
            (0.0..=1.0).contains(&ndc.z),
            "inside the depth range: {}",
            ndc.z
        );
    }
}
