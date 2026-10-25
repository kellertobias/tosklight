//! Existing shaded rendering resources, initialized only when a shaded/plan view is used.
use super::context::FrameContext;
use super::*;

pub(super) struct ShadedRenderer {
    pub(super) draft: Option<super::draft::DraftPasses>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) size: (u32, u32),
    pub(super) samples: u32,
    pub(super) targets: Targets,
    /// Fraction of the surface the shaded passes are drawn at, from the active quality tier. The
    /// composite samples the result up to the surface, so the overlay and the plan views stay at
    /// the display's own resolution whatever this is.
    pub(super) target_scale: f32,
    pub(super) meshes: HashMap<MeshKind, GpuMesh>,
    asset_cache: super::shaded_assets::AssetCache,
    assets_changed: bool,
    pub(super) cone: GpuMesh,
    pub(super) mesh_instances: HashMap<MeshKind, DynamicBuffer>,
    pub(super) beam_instances: DynamicBuffer,
    pub(super) laser_instances: DynamicBuffer,
    pub(super) line_vertices: DynamicBuffer,
    pub(super) lights: DynamicBuffer,
    pub(super) curtain_blockers: DynamicBuffer,
    pub(super) tile_counts: DynamicBuffer,
    pub(super) tile_lights: DynamicBuffer,
    pub(super) globals_buffer: wgpu::Buffer,
    pub(super) post_settings: wgpu::Buffer,
    pub(super) scene_layout: wgpu::BindGroupLayout,
    pub(super) cull_layout: wgpu::BindGroupLayout,
    pub(super) depth_layout: wgpu::BindGroupLayout,
    pub(super) post_layout: wgpu::BindGroupLayout,
    pub(super) composite_layout: wgpu::BindGroupLayout,
    pub(super) scene_bind_group: wgpu::BindGroup,
    pub(super) cull_bind_group: wgpu::BindGroup,
    pub(super) depth_bind_group: wgpu::BindGroup,
    pub(super) shadow_atlas: wgpu::TextureView,
    pub(super) shadow_pipeline: wgpu::RenderPipeline,
    pub(super) shadow_layout: wgpu::BindGroupLayout,
    pub(super) shadow_draw_layout: wgpu::BindGroupLayout,
    pub(super) shadow_sampler: wgpu::Sampler,
    /// Every piece of gobo artwork in the rig, as one array the lighting passes sample.
    pub(super) gobo_atlas: crate::gobos::GoboAtlas,
    pub(super) gobo_sampler: wgpu::Sampler,
    pub(super) shadow_bind_group: wgpu::BindGroup,
    pub(super) shadow_draw_group: wgpu::BindGroup,
    pub(super) shadow_matrices: DynamicBuffer,
    pub(super) shadow_draws: wgpu::Buffer,
    /// How many lights actually have a map this frame.
    pub(super) shadow_count: u32,
    /// Geometry/light identity represented by the current shadow atlas. Media pixels are excluded,
    /// so a new video frame never redraws unchanged projector or fixture shadow maps.
    pub(super) shadow_signature: Option<u64>,
    pub(super) haze_view: wgpu::TextureView,
    pub(super) haze_sampler: wgpu::Sampler,
    pub(super) bloom_extract_group: wgpu::BindGroup,
    pub(super) bloom_blur_a_group: wgpu::BindGroup,
    pub(super) bloom_blur_b_group: wgpu::BindGroup,
    pub(super) composite_source_group: wgpu::BindGroup,
    pub(super) composite_bloom_group: wgpu::BindGroup,
    pub(super) sampler: wgpu::Sampler,
    pub(super) surface_pipeline: wgpu::RenderPipeline,
    pub(super) line_pipeline: wgpu::RenderPipeline,
    pub(super) beam_pipeline: wgpu::RenderPipeline,
    pub(super) laser_pipeline: wgpu::RenderPipeline,
    pub(super) cull_pipeline: wgpu::ComputePipeline,
    pub(super) bounds_pipeline: wgpu::ComputePipeline,
    pub(super) screen_bounds: super::tiled::ScreenBounds,
    pub(super) extract_pipeline: wgpu::RenderPipeline,
    pub(super) blur_pipeline: wgpu::RenderPipeline,
    pub(super) composite_pipeline: wgpu::RenderPipeline,
    pub(super) media_panel_instances: DynamicBuffer,
    pub(super) media_pipeline: wgpu::RenderPipeline,
    pub(super) media_bind_group: wgpu::BindGroup,
    pub(super) frame: FrameInstances,
    pub(super) media_content_enabled: bool,
    /// Whether the beam pass had to skip volumes to hold the budget.
    pub(super) beam_overflow: bool,
    pub(super) ultra_budget: UltraBudget,
    pub(super) capture_quality: bool,
    pub(super) crowd_amount: f32,
}

impl ShadedRenderer {
    pub(super) fn new(
        context: &FrameContext<'_>,
        samples: u32,
        media_atlas: &crate::media::MediaAtlas,
    ) -> Self {
        Self::new_with_mode(context, samples, media_atlas, false)
    }

    pub(super) fn new_draft(
        context: &FrameContext<'_>,
        media_atlas: &crate::media::MediaAtlas,
    ) -> Self {
        Self::new_with_mode(context, 1, media_atlas, true)
    }

    fn new_with_mode(
        context: &FrameContext<'_>,
        samples: u32,
        media_atlas: &crate::media::MediaAtlas,
        draft_mode: bool,
    ) -> Self {
        let device = context.device.clone();
        let queue = context.queue.clone();
        let targets = if draft_mode {
            Targets::new_draft(&device, context.size.0, context.size.1)
        } else {
            Targets::new(&device, context.size.0, context.size.1, samples)
        };
        let multisample = wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        };

        let mut meshes = HashMap::new();
        for kind in MeshKind::PROCEDURAL {
            let Some((name, data)) = mesh::procedural(kind) else {
                continue;
            };
            meshes.insert(kind, GpuMesh::new(&device, name, &data));
        }
        let cone = GpuMesh::new(&device, "cone", &mesh::unit_cone(28));

        let buffers = SceneBuffers::new(&device, &queue, &targets);

        // What the shaders may see, what they are compiled from, and the pipelines that result.
        let layouts = Layouts::new(&device, samples);
        let pipeline_layouts = PipelineLayouts::new(&device, &layouts);
        let modules = Modules::new(&device, samples, draft_mode);
        let pipelines = Pipelines::new(
            &device,
            &layouts,
            &pipeline_layouts,
            &modules,
            multisample,
            context.format,
            draft_mode,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viz linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow = ShadowResources::new(&device);
        let groups = Groups::new(
            &device, &queue, &layouts, &buffers, &shadow, &targets, &sampler,
        );
        let media_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viz media sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let media_bind_group =
            build_media_group(&device, &layouts.media, &media_atlas.view, &media_sampler);

        let draft = draft_mode.then(|| {
            super::draft::DraftPasses::new(
                &device,
                context.size,
                &layouts.scene,
                &buffers,
                &targets,
                context.format,
            )
        });
        let screen_bounds = super::tiled::ScreenBounds::new(&device, &layouts.bounds);
        Self {
            draft,
            device,
            queue,
            size: context.size,
            samples,
            targets,
            target_scale: 1.0,
            meshes,
            asset_cache: super::shaded_assets::AssetCache::default(),
            assets_changed: false,
            cone,
            mesh_instances: buffers.mesh_instances,
            beam_instances: buffers.beam_instances,
            laser_instances: buffers.laser_instances,
            line_vertices: buffers.line_vertices,
            lights: buffers.lights,
            curtain_blockers: buffers.curtain_blockers,
            tile_counts: buffers.tile_counts,
            tile_lights: buffers.tile_lights,
            globals_buffer: buffers.globals,
            post_settings: buffers.post_settings,
            scene_layout: layouts.scene,
            cull_layout: layouts.cull,
            depth_layout: layouts.depth,
            post_layout: layouts.post,
            composite_layout: layouts.composite,
            scene_bind_group: groups.scene,
            cull_bind_group: groups.cull,
            depth_bind_group: groups.depth,
            shadow_atlas: shadow.atlas,
            shadow_pipeline: pipelines.shadow,
            shadow_layout: layouts.shadow,
            shadow_draw_layout: layouts.shadow_draw,
            shadow_sampler: shadow.sampler,
            gobo_atlas: groups.gobo_atlas,
            gobo_sampler: groups.gobo_sampler,
            shadow_bind_group: groups.shadow,
            shadow_draw_group: groups.shadow_draw,
            shadow_matrices: shadow.matrices,
            shadow_draws: shadow.draws,
            shadow_count: 0,
            shadow_signature: None,
            haze_view: groups.haze_view,
            haze_sampler: groups.haze_sampler,
            bloom_extract_group: groups.bloom_extract,
            bloom_blur_a_group: groups.bloom_blur_a,
            bloom_blur_b_group: groups.bloom_blur_b,
            composite_source_group: groups.composite_source,
            composite_bloom_group: groups.composite_bloom,
            sampler,
            surface_pipeline: pipelines.surface,
            line_pipeline: pipelines.line,
            beam_pipeline: pipelines.beam,
            laser_pipeline: pipelines.laser,
            cull_pipeline: pipelines.cull,
            bounds_pipeline: pipelines.bounds,
            screen_bounds,
            extract_pipeline: pipelines.extract,
            blur_pipeline: pipelines.blur,
            composite_pipeline: pipelines.composite,
            media_panel_instances: buffers.media_panels,
            media_pipeline: pipelines.media,
            media_bind_group,
            frame: FrameInstances::default(),
            media_content_enabled: true,
            beam_overflow: false,
            ultra_budget: UltraBudget::default(),
            capture_quality: false,
            crowd_amount: 1.0,
        }
    }

    pub(super) fn resize(&mut self, size: (u32, u32)) {
        if self.size != size {
            self.size = size;
            self.rebuild_targets();
        }
    }

    pub(super) fn observe_gpu_micros(&mut self, micros: u64) {
        if !self.capture_quality {
            self.ultra_budget.observe(micros);
        }
    }

    pub(super) fn assets_changed(&self) -> bool {
        self.assets_changed
    }

    pub(super) fn render_size(&self) -> (u32, u32) {
        (self.targets.width, self.targets.height)
    }

    pub(super) fn set_preferences(&mut self, crowd_amount: f32, media_content: bool) {
        self.crowd_amount = crowd_amount;
        self.media_content_enabled = media_content;
    }

    /// Adopt the quality tier's render scale. Rebuilding every target is far too expensive to do
    /// per frame, so it happens only when the scale actually changes — a quality change or a
    /// resize, both of which an operator does deliberately.
    pub(super) fn set_target_scale(&mut self, scale: f32) {
        let scale = scale.clamp(0.25, 2.0);
        if (scale - self.target_scale).abs() < 0.001 {
            return;
        }
        self.target_scale = scale;
        self.rebuild_targets();
    }

    /// The size the shaded passes draw at: the surface scaled by the active tier.
    pub(super) fn scaled_size(&self) -> (u32, u32) {
        let scale = self.target_scale;
        let width = ((self.size.0 as f32 * scale).round() as u32).max(1);
        let height = ((self.size.1 as f32 * scale).round() as u32).max(1);
        (width, height)
    }

    pub(super) fn rebuild_targets(&mut self) {
        let device = self.device.clone();
        let (width, height) = self.scaled_size();
        self.targets = if self.draft.is_some() {
            Targets::new_draft(&device, width, height)
        } else {
            Targets::new(&device, width, height, self.samples)
        };
        if let Some(draft) = self.draft.as_mut() {
            draft.resize(&device, &self.targets);
        }
        let tiles = self.targets.tile_count();
        self.tile_counts
            .ensure(&device, (tiles * 4).max(256) as u64);
        self.tile_lights
            .ensure(&device, (tiles * MAX_LIGHTS_PER_TILE * 4).max(256) as u64);
        self.rebuild_scene_groups();
        self.depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz depth"),
            layout: &self.depth_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.targets.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.haze_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.haze_sampler),
                },
            ],
        });
        self.bloom_extract_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.shaded,
            &self.sampler,
            &self.post_settings,
        );
        self.bloom_blur_a_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.bloom_a,
            &self.sampler,
            &self.post_settings,
        );
        self.bloom_blur_b_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.bloom_b,
            &self.sampler,
            &self.post_settings,
        );
        self.composite_source_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.shaded,
            &self.sampler,
            &self.post_settings,
        );
        self.composite_bloom_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz composite bloom"),
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.targets.bloom_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
    }

    /// Rebuild the two groups that point at the shadow matrix buffer, which moves whenever the
    /// number of shadow-casting lights grows past its allocation.
    /// Every mesh the current frame asked to draw.
    pub(super) fn drawn_meshes(&self) -> Vec<MeshKind> {
        self.frame
            .meshes
            .iter()
            .filter(|(_, instances)| !instances.is_empty())
            .map(|(kind, _)| *kind)
            .collect()
    }

    /// Replace changed content and release missing scene-owned slots before shadow assignment.
    pub(super) fn ensure_model_meshes(&mut self, scene: &Scene) {
        let changes = self.asset_cache.reconcile(scene);
        self.assets_changed = !changes.changed.is_empty() || !changes.removed.is_empty();
        if self.assets_changed {
            self.shadow_signature = None;
        }
        for kind in changes.removed {
            self.meshes.remove(&kind);
            self.mesh_instances.remove(&kind);
        }
        for kind in changes.changed {
            let Some(data) = super::shaded_assets::mesh_data(scene, kind) else {
                continue;
            };
            self.meshes
                .insert(kind, GpuMesh::new(&self.device, "scene asset", &data));
            self.mesh_instances.entry(kind).or_insert_with(|| {
                DynamicBuffer::new(
                    &self.device,
                    "scene asset instances",
                    BufferUsages::VERTEX,
                    (size_of::<MeshInstance>() * 256) as u64,
                )
            });
        }
    }

    /// Upload the scene's gobo artwork when it is not the artwork already on the GPU.
    ///
    /// A rig's glass changes when the show does — a repatch, another profile, a new revision —
    /// and not otherwise, so this is keyed on the scene revision rather than run per frame.
    pub(super) fn ensure_gobo_atlas(&mut self, scene: &Scene) {
        if self.gobo_atlas.matches(&scene.gobo_artwork, scene.revision) {
            return;
        }
        self.gobo_atlas = crate::gobos::GoboAtlas::new(
            &self.device,
            &self.queue,
            &scene.gobo_artwork,
            scene.revision,
        );
        self.rebuild_shadow_groups();
    }

    pub(super) fn rebuild_shadow_groups(&mut self) {
        let device = self.device.clone();
        self.shadow_bind_group = build_shadow_group(
            &device,
            &self.shadow_layout,
            &self.shadow_atlas,
            &self.shadow_sampler,
            &self.shadow_matrices,
            &self.gobo_atlas.view,
            &self.gobo_sampler,
            &self.curtain_blockers,
        );
        self.shadow_draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz shadow draw"),
            layout: &self.shadow_draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shadow_matrices.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.shadow_draws,
                        offset: 0,
                        size: std::num::NonZeroU64::new(16),
                    }),
                },
            ],
        });
    }

    pub(super) fn rebuild_scene_groups(&mut self) {
        let device = self.device.clone();
        self.scene_bind_group = build_scene_group(
            &device,
            &self.scene_layout,
            &self.globals_buffer,
            &self.lights,
            &self.tile_counts,
            &self.tile_lights,
        );
        self.cull_bind_group = build_scene_group(
            &device,
            &self.cull_layout,
            &self.globals_buffer,
            &self.lights,
            &self.tile_counts,
            &self.tile_lights,
        );
        if let Some(draft) = self.draft.as_mut() {
            draft.scene = build_scene_group(
                &device,
                &self.scene_layout,
                &draft.globals,
                &self.lights,
                &self.tile_counts,
                &self.tile_lights,
            );
        }
    }
}
