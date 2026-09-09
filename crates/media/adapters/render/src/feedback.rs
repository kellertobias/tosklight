//! Per-layer temporal feedback.
//!
//! Feedback is prepared in source space before the ordinary ordered layer effects and geometry.
//! Two textures are alternated so a pass never samples the texture it is writing. Removing or
//! bypassing the effect marks the history inactive; the next enable starts from the live frame
//! instead of resurrecting stale pixels.

use bytemuck::{Pod, Zeroable};
use media_domain::geometry::Size;
use media_domain::{FeedbackParameters, LayerState, MediaAddress, Timestamp};
use std::collections::{HashMap, HashSet};

use crate::{Gpu, LayerDraw, MAX_LAYERS, SourceTexture, texture::VISUALIZER_FORMAT};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FeedbackUniform {
    amount: f32,
    motion: f32,
    direction: f32,
    reset: f32,
    delta_seconds: f32,
    clock_seconds: f32,
    _padding: [f32; 2],
}

struct History {
    textures: [SourceTexture; 2],
    source_size: Size,
    current: usize,
    active: bool,
    address: MediaAddress,
    last_frame: Option<Timestamp>,
}

impl History {
    fn new(gpu: &Gpu, size: Size, address: MediaAddress) -> Self {
        // Temporal feedback does not need source-resolution detail. Half-resolution ping-pong
        // targets cut both retained GPU memory and feedback fill work to one quarter.
        let target_size = feedback_target_size(size);
        let target = || {
            SourceTexture::render_target(gpu, target_size)
                .expect("a feedback target matches an already accepted source size")
        };
        Self {
            textures: [target(), target()],
            source_size: size,
            current: 0,
            active: false,
            address,
            last_frame: None,
        }
    }

    fn matches(&self, size: Size, address: MediaAddress) -> bool {
        self.source_size == size && self.address == address
    }
}

pub(crate) struct FeedbackProcessor {
    gpu: Gpu,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniforms: Vec<wgpu::Buffer>,
    histories: HashMap<u64, History>,
}

impl FeedbackProcessor {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let layout = layout(&gpu.device);
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("media-feedback"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/feedback.wgsl").into()),
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("media-feedback"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("media-feedback"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: VISUALIZER_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });
        let uniforms = (0..MAX_LAYERS)
            .map(|index| {
                gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("media-feedback-{index}")),
                    size: std::mem::size_of::<FeedbackUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        Self {
            gpu: gpu.clone(),
            pipeline,
            layout,
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("media-feedback"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            uniforms,
            histories: HashMap::new(),
        }
    }

    pub(crate) fn advance(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        layers: &[LayerDraw<'_>],
        now: Timestamp,
    ) {
        let mut active = HashSet::new();
        for (index, layer) in layers.iter().take(MAX_LAYERS).enumerate() {
            let Some((parameters, mix_amount, seed)) = feedback(layer.state) else {
                continue;
            };
            let size = layer.source.size();
            let address = layer.state.address;
            let key = history_key(address, seed);
            active.insert(key);
            if !self
                .histories
                .get(&key)
                .is_some_and(|history| history.matches(size, address))
            {
                self.histories
                    .insert(key, History::new(&self.gpu, size, address));
            }
            let history = self.histories.get_mut(&key).expect("history was created");
            let reset = !history.active;
            let delta_seconds = history
                .last_frame
                .map(|previous| now.since(previous).as_secs_f32().clamp(0.0, 0.1))
                .unwrap_or(0.0);
            let destination = 1 - history.current;
            let uniform = FeedbackUniform {
                amount: parameters.amount * mix_amount,
                motion: parameters.motion,
                direction: parameters.direction.parameter(),
                reset: f32::from(u8::from(reset)),
                delta_seconds,
                // A bounded clock preserves sub-frame precision after days of uptime.
                clock_seconds: (now.as_micros() % 1_000_000_000) as f32 / 1_000_000.0,
                _padding: [0.0; 2],
            };
            self.gpu
                .queue
                .write_buffer(&self.uniforms[index], 0, bytemuck::bytes_of(&uniform));
            let group = self
                .gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("media-feedback"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.uniforms[index].as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&layer.source.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(
                                &history.textures[history.current].view,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("media-feedback"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &history.textures[destination].view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..6, 0..1);
            drop(pass);
            history.current = destination;
            history.active = true;
            history.last_frame = Some(now);
        }
        // Release two GPU textures as soon as a feedback path is bypassed. Re-enabling starts
        // from live pixels already, so retaining inactive histories only consumed GPU memory.
        self.histories.retain(|key, _| active.contains(key));
    }

    pub(crate) fn source<'a>(&'a self, layer: &'a LayerDraw<'_>) -> &'a wgpu::TextureView {
        let key = feedback(layer.state).map(|(_, _, seed)| history_key(layer.state.address, seed));
        self.histories
            .get(&key.unwrap_or_default())
            .filter(|history| history.active && key.is_some())
            .map(|history| &history.textures[history.current].view)
            .unwrap_or(&layer.source.view)
    }
}

fn feedback(layer: &LayerState) -> Option<(FeedbackParameters, f32, u32)> {
    layer.effects.iter().find_map(|effect| {
        effect
            .feedback_parameters()
            .map(|parameters| (parameters, effect.mix.clamp(0.0, 1.0), effect.seed))
    })
}

fn history_key(address: MediaAddress, seed: u32) -> u64 {
    (u64::from(seed) << 16) | (u64::from(address.folder) << 8) | u64::from(address.file)
}

fn feedback_target_size(source: Size) -> Size {
    Size::new(source.width.div_ceil(2), source.height.div_ceil(2))
}

fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("media-feedback"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{EffectSlot, FeedbackMotion};

    #[test]
    fn bypassed_feedback_is_not_selected_for_processing() {
        let mut effect = EffectSlot::feedback();
        effect.enabled = false;
        let mut effects: [EffectSlot; 4] = Default::default();
        effects[0] = effect;
        assert!(
            feedback(&LayerState {
                effects,
                ..Default::default()
            })
            .is_none()
        );
    }

    #[test]
    fn all_motion_modes_have_stable_shader_values() {
        for (index, direction) in FeedbackMotion::ALL.into_iter().enumerate() {
            assert_eq!(direction.parameter(), index as f32);
            assert_eq!(FeedbackMotion::from_parameter(index as f32), direction);
        }
    }

    #[test]
    fn feedback_history_uses_one_quarter_the_source_pixels() {
        assert_eq!(
            feedback_target_size(Size::new(1920, 1080)),
            Size::new(960, 540)
        );
        assert_eq!(feedback_target_size(Size::new(1, 3)), Size::new(1, 2));
    }

    #[test]
    fn shake_and_tunnel_feedback_shader_is_valid_wgsl() {
        let module = naga::front::wgsl::parse_str(include_str!("shaders/feedback.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
    }
}
