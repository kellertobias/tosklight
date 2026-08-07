//! Drawing the renderer's pane into the desk's own window.
//!
//! The window is a native window with the interface added on top as a transparent child webview,
//! so there is a surface underneath everything the desk draws. This is what draws on it: one
//! picture, in one rectangle, with the whole rest of the surface left transparent so the interface
//! above is untouched.
//!
//! The scissor is the contract rather than a convention. The renderer is given a rectangle and this
//! draws inside it and nowhere else, so a menu opening across the pane, a dialog over it, or the
//! sheet beside it are drawn by the webview above and physically cannot be painted over.
//!
//! Where the picture comes from depends on what the two processes agreed. A shared surface is
//! sampled where it lies. Copied frames are uploaded into a texture of the desk's own. Either way
//! what reaches the draw below is a texture, and the draw does not care which it was.

use std::sync::Arc;
use viz_helper::pane::PaneRect;

/// The blit.
///
/// A fullscreen triangle rather than a quad: one primitive and no seam down the diagonal. The
/// viewport is what puts it in the pane, so the vertex positions never mention the rectangle —
/// but `@builtin(position)` is in the surface's coordinates rather than the viewport's, so the
/// fragment subtracts the pane's origin to find where in the picture it is.
const SHADER: &str = r"
struct Pane {
    origin: vec2<f32>,
    size: vec2<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> pane: Pane;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (position.xy - pane.origin) / pane.size;
    return textureSample(picture, picture_sampler, uv);
}
";

/// What the desk draws, and where.
pub struct StageCompositor {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    /// Physical pixel size of the whole window surface.
    surface_size: (u32, u32),
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Where the pane is, in the surface's pixels, as the shader reads it.
    uniform: wgpu::Buffer,
    source: Option<Source>,
    pane: PaneRect,
    scale: f32,
}

/// The picture to draw, however it arrived.
struct Source {
    /// Held for a shared surface so its memory outlives the texture over it.
    shared: Option<viz_surface::SharedSurface>,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

impl StageCompositor {
    /// Attach to the desk's window.
    ///
    /// `target` is the native window the interface sits on top of. Everything drawn here is beneath
    /// that interface, which is the arrangement the embedded pane depends on entirely.
    pub fn attach<T>(target: Arc<T>, width: u32, height: u32, scale: f32) -> Result<Self, String>
    where
        T: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .map_err(|error| format!("the desk's window has no drawable surface: {error}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|error| format!("no adapter can draw on the desk's window: {error}"))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|error| format!("the desk's window could not open a device: {error}"))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("the desk's window offers no surface format")?;
        // Everything outside the pane is left fully transparent, and it is the window's own
        // transparency that lets the interface show through there. A platform without an alpha
        // mode still draws the pane; the rest of the window is simply opaque, which is the desk's
        // background colour rather than a hole.
        let alpha_mode = if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            capabilities.alpha_modes[0]
        };

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage pane"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stage pane"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stage pane"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stage pane"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stage pane"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stage pane"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut compositor = Self {
            surface,
            device,
            queue,
            format,
            alpha_mode,
            surface_size: (width.max(1), height.max(1)),
            pipeline,
            layout,
            sampler,
            uniform,
            source: None,
            pane: PaneRect::default(),
            scale: if scale > 0.0 { scale } else { 1.0 },
        };
        compositor.configure();
        Ok(compositor)
    }

    /// Where the desk's layout says the Stage pane is, in the points the web layout works in.
    pub fn set_pane(&mut self, pane: PaneRect, scale: f32) {
        self.pane = pane;
        if scale > 0.0 {
            self.scale = scale;
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let size = (width.max(1), height.max(1));
        if size == self.surface_size {
            return;
        }
        self.surface_size = size;
        self.configure();
    }

    /// Adopt a surface the renderer created and both processes address.
    pub fn adopt_shared(
        &mut self,
        handle: viz_helper::protocol::SharedSurfaceHandle,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let shared = viz_surface::import(&self.device, handle, width, height)
            .map_err(|error| error.to_string())?;
        let texture = shared.texture().clone();
        let bind_group = self.bind(&texture);
        self.source = Some(Source {
            shared: Some(shared),
            texture,
            bind_group,
            size: (width, height),
        });
        Ok(())
    }

    /// Take a frame that came through the pipe, uploading it into a texture of the desk's own.
    pub fn accept_copy(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
        let expected = (width as usize) * (height as usize) * 4;
        if width == 0 || height == 0 || rgba.len() < expected {
            return Err("the renderer sent fewer pixels than the frame it announced".to_owned());
        }
        let reusable = self
            .source
            .as_ref()
            .is_some_and(|source| source.size == (width, height) && source.shared.is_none());
        if !reusable {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("stage pane copy"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: viz_surface::FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let bind_group = self.bind(&texture);
            self.source = Some(Source {
                shared: None,
                texture,
                bind_group,
                size: (width, height),
            });
        }
        let Some(source) = self.source.as_ref() else {
            return Ok(());
        };
        self.queue.write_texture(
            source.texture.as_image_copy(),
            &rgba[..expected],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    /// Forget whatever was being drawn, leaving the whole window to the interface again.
    pub fn clear_source(&mut self) {
        self.source = None;
    }

    /// Draw one frame: the pane where the layout put it, and nothing anywhere else.
    pub fn draw(&mut self) -> Result<(), String> {
        use wgpu::CurrentSurfaceTexture;
        let frame = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) | CurrentSurfaceTexture::Suboptimal(frame) => {
                frame
            }
            // A surface goes out of date while a display is reconfigured or the window moves
            // between screens. Reconfiguring and skipping one frame is the whole recovery.
            CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
                self.configure();
                return Ok(());
            }
            // Occluded is the window being hidden and timeout is the compositor being busy.
            // Neither is a fault, and neither has a frame to draw into.
            CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => return Ok(()),
            CurrentSurfaceTexture::Validation => {
                return Err("the desk's window refused a frame".to_owned());
            }
        };
        let viewport = self.viewport();
        if let Some(pane) = viewport {
            let origin_and_size: [f32; 4] =
                [pane.0 as f32, pane.1 as f32, pane.2 as f32, pane.3 as f32];
            self.queue
                .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&origin_and_size));
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stage pane"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stage pane"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        // Everything outside the pane is cleared to nothing at all, so what the
                        // operator sees there is the interface drawn above.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let (Some(source), Some(pane)) = (self.source.as_ref(), viewport) {
                pass.set_viewport(
                    pane.0 as f32,
                    pane.1 as f32,
                    pane.2 as f32,
                    pane.3 as f32,
                    0.0,
                    1.0,
                );
                pass.set_scissor_rect(pane.0, pane.1, pane.2, pane.3);
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &source.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }

    /// The pane in the surface's own pixels, or `None` when there is nothing to draw.
    fn viewport(&self) -> Option<(u32, u32, u32, u32)> {
        let pixels = self
            .pane
            .to_pixels(self.scale, self.surface_size.0, self.surface_size.1)?;
        Some((pixels.x, pixels.y, pixels.width, pixels.height))
    }

    fn bind(&self, texture: &wgpu::Texture) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage pane"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        })
    }

    fn configure(&mut self) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                width: self.surface_size.0,
                height: self.surface_size.1,
                present_mode: wgpu::PresentMode::AutoVsync,
                // Whatever the display is already showing the interface in. The pane must match
                // the chrome around it, and the desk does not manage colour itself.
                color_space: wgpu::SurfaceColorSpace::Auto,
                alpha_mode: self.alpha_mode,
                view_formats: Vec::new(),
                desired_maximum_frame_latency: 2,
            },
        );
    }
}
