//! Presenting to the window, inside the rectangle the web layout says is the pane.
//!
//! Two things are being demonstrated here and nothing else. First, that the renderer can confine
//! itself to an arbitrary sub-rectangle of the window with a scissor, so the surrounding chrome is
//! genuinely untouched rather than merely covered. Second, that the pixels outside the pane can be
//! left fully transparent, so whatever the webview draws there is what is seen.
//!
//! The scene is a stand-in. It animates, it responds to the camera, and it is obviously a rendered
//! image rather than a web one — that is all it has to be.

use std::{sync::Arc, time::Instant};

use bytemuck::{Pod, Zeroable};

use crate::state::Shared;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    /// Pane rectangle in physical pixels: x, y, width, height.
    pane: [f32; 4],
    /// Camera yaw, pitch, distance, and elapsed seconds.
    camera: [f32; 4],
}

pub struct Renderer {
    window: Arc<tauri::Window>,
    shared: Arc<Shared>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    started_at: Instant,
}

impl Renderer {
    pub async fn new(
        window: Arc<tauri::Window>,
        shared: Arc<Shared>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use wgpu::util::DeviceExt;

        let size = window.inner_size()?;
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("embedded pane device"),
                ..Default::default()
            })
            .await?;

        let width = size.width.max(1);
        let height = size.height.max(1);
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or("the selected GPU cannot present to the Tauri window")?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("embedded pane uniforms"),
            contents: bytemuck::bytes_of(&Uniforms {
                pane: [0.0, 0.0, width as f32, height as f32],
                camera: [0.6, 0.35, 1.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embedded pane uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("embedded pane uniforms"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("embedded pane shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("embedded pane pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("embedded pane pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
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

        Ok(Self {
            window,
            shared,
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            started_at: Instant::now(),
        })
    }

    pub fn render(&mut self) {
        let Ok(size) = self.window.inner_size() else {
            return;
        };
        let width = size.width.max(1);
        let height = size.height.max(1);
        if width != self.config.width || height != self.config.height {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }

        // The pane arrives in logical points, because that is what the web layout deals in. The
        // surface is in physical pixels. Converting here rather than on the web side is the whole
        // of "DPI correctness" for this arrangement, and it has to survive a window moving between
        // a Retina display and an external one while it is open.
        let scale = self.window.scale_factor().unwrap_or(1.0) as f32;
        let (pane, camera) = self.shared.read();
        let pane_x = (pane.x * scale).round().max(0.0);
        let pane_y = (pane.y * scale).round().max(0.0);
        let pane_width = (pane.width * scale)
            .round()
            .clamp(0.0, width as f32 - pane_x);
        let pane_height = (pane.height * scale)
            .round()
            .clamp(0.0, height as f32 - pane_y);
        if pane_width < 1.0 || pane_height < 1.0 {
            // The web side has not laid out yet, or the pane is collapsed. Presenting a frame that
            // clears the whole window would flash over the chrome, so nothing is drawn at all.
            return;
        }

        let elapsed = self.started_at.elapsed().as_secs_f32();
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                pane: [pane_x, pane_y, pane_width, pane_height],
                camera: [camera.yaw, camera.pitch, camera.distance, elapsed],
            }),
        );

        // An outdated or lost surface is the normal consequence of a resize or a display change,
        // and the next frame gets it right; a dropped frame is not worth a reconfigure storm here.
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("embedded pane encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("embedded pane pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Everything outside the pane is cleared to nothing, so the window is the
                        // webview's except where the pane is. This is what makes the arrangement
                        // a pane rather than a backdrop.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // The scissor is the contract: the renderer physically cannot write a pixel outside
            // the rectangle the web layout gave it.
            pass.set_scissor_rect(
                pane_x as u32,
                pane_y as u32,
                pane_width as u32,
                pane_height as u32,
            );
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        self.shared.count_frame();
    }
}
