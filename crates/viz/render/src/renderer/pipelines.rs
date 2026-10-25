//! The shader modules and the pipelines built from them.
//!
//! One function per pipeline, each saying in its own place what that pass is: what it reads, how
//! it blends, and whether it takes part in the depth buffer. Read together they are the whole
//! drawing order of a frame.

use super::layouts::{Layouts, PipelineLayouts};
use super::{
    BEAM_WGSL, COMMON_WGSL, CULL_WGSL, LASER_WGSL, LINES_WGSL, MEDIA_WGSL, POST_WGSL,
    SCENE_DEPTH_BINDING, SHADOW_PRELUDE_WGSL, SHADOW_WGSL, SURFACE_WGSL, scene_depth_binding,
};
use crate::instances::{BeamInstance, GpuMediaPanel, LaserInstance, LineVertex, MeshInstance};
use crate::mesh::Vertex;
use crate::targets::{DEPTH_FORMAT, HDR_FORMAT};

const BEAM_BLEND: wgpu::BlendState = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;

/// Every compiled shader module, built once at startup.
pub(super) struct Modules {
    pub surface: wgpu::ShaderModule,
    pub media: wgpu::ShaderModule,
    pub beam: wgpu::ShaderModule,
    pub laser: wgpu::ShaderModule,
    pub line: wgpu::ShaderModule,
    pub cull: wgpu::ShaderModule,
    pub post: wgpu::ShaderModule,
    pub shadow: wgpu::ShaderModule,
}

impl Modules {
    pub(super) fn new(device: &wgpu::Device, samples: u32, draft: bool) -> Self {
        let surface_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz surface"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{COMMON_WGSL}\n{}",
                    if draft {
                        include_str!("../shaders/draft_surface.wgsl")
                    } else {
                        SURFACE_WGSL
                    }
                )
                .into(),
            ),
        });
        let media_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz media"),
            source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}\n{MEDIA_WGSL}").into()),
        });
        // The beam pass reads the depth the geometry left behind. Multisampled depth is a
        // different WGSL type, so the declaration follows the sample count the adapter gave us;
        // the load itself is written once and takes sample 0 either way.
        let beam_source = if draft {
            format!(
                "{}\n{}",
                include_str!("../shaders/draft_integral.wgsl"),
                include_str!("../shaders/draft_beam.wgsl")
            )
        } else {
            BEAM_WGSL.to_owned()
        };
        let beam_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz beam"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{COMMON_WGSL}\n{beam_source}")
                    .replace(SCENE_DEPTH_BINDING, scene_depth_binding(samples))
                    .into(),
            ),
        });
        // A laser reads the same depth for the same reason the beam pass does, so it follows the
        // same multisample-dependent declaration.
        let laser_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz laser"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{COMMON_WGSL}\n{LASER_WGSL}")
                    .replace(SCENE_DEPTH_BINDING, scene_depth_binding(samples))
                    .into(),
            ),
        });
        let line_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz lines"),
            source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}\n{LINES_WGSL}").into()),
        });
        let cull_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz cull"),
            source: wgpu::ShaderSource::Wgsl(CULL_WGSL.into()),
        });
        let post_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz post"),
            source: wgpu::ShaderSource::Wgsl(POST_WGSL.into()),
        });
        let shadow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz shadow"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{SHADOW_PRELUDE_WGSL}\n{SHADOW_WGSL}").into(),
            ),
        });
        Self {
            surface: surface_module,
            media: media_module,
            beam: beam_module,
            laser: laser_module,
            line: line_module,
            cull: cull_module,
            post: post_module,
            shadow: shadow_module,
        }
    }
}

/// Every pipeline a frame can use.
pub(super) struct Pipelines {
    pub surface: wgpu::RenderPipeline,
    pub media: wgpu::RenderPipeline,
    pub line: wgpu::RenderPipeline,
    pub beam: wgpu::RenderPipeline,
    pub laser: wgpu::RenderPipeline,
    pub shadow: wgpu::RenderPipeline,
    pub cull: wgpu::ComputePipeline,
    pub bounds: wgpu::ComputePipeline,
    pub extract: wgpu::RenderPipeline,
    pub blur: wgpu::RenderPipeline,
    pub composite: wgpu::RenderPipeline,
}

impl Pipelines {
    pub(super) fn new(
        device: &wgpu::Device,
        _layouts: &Layouts,
        pipeline_layouts: &PipelineLayouts,
        modules: &Modules,
        multisample: wgpu::MultisampleState,
        surface_format: wgpu::TextureFormat,
        draft_mode: bool,
    ) -> Self {
        Self {
            surface: surface_pipeline(device, pipeline_layouts, modules, multisample),
            media: media_pipeline(device, pipeline_layouts, modules, multisample),
            line: line_pipeline(device, pipeline_layouts, modules, multisample),
            beam: beam_pipeline(device, pipeline_layouts, modules, multisample),
            laser: laser_pipeline(device, pipeline_layouts, modules, multisample),
            shadow: shadow_pipeline(device, pipeline_layouts, modules),
            cull: cull_pipeline(device, pipeline_layouts, modules, draft_mode, "cull_main"),
            bounds: cull_pipeline(device, pipeline_layouts, modules, draft_mode, "bounds_main"),
            extract: fullscreen_pipeline(
                device,
                "viz bloom extract",
                &pipeline_layouts.post,
                &modules.post,
                "extract",
                HDR_FORMAT,
            ),
            blur: fullscreen_pipeline(
                device,
                "viz bloom blur",
                &pipeline_layouts.post,
                &modules.post,
                "blur",
                HDR_FORMAT,
            ),
            composite: fullscreen_pipeline(
                device,
                "viz composite",
                &pipeline_layouts.composite,
                &modules.post,
                "composite",
                surface_format,
            ),
        }
    }
}

fn media_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
    multisample: wgpu::MultisampleState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz media"),
        layout: Some(&layouts.surface),
        vertex: wgpu::VertexState {
            module: &modules.media,
            entry_point: Some("media_vertex"),
            buffers: &[Some(Vertex::LAYOUT), Some(GpuMediaPanel::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &modules.media,
            entry_point: Some("media_fragment"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample,
        multiview_mask: None,
        cache: None,
    })
}

fn surface_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
    multisample: wgpu::MultisampleState,
) -> wgpu::RenderPipeline {
    let (surface_pipeline_layout, surface_module) = (&layouts.surface, &modules.surface);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz surface"),
        layout: Some(surface_pipeline_layout),
        vertex: wgpu::VertexState {
            module: surface_module,
            entry_point: Some("vertex_main"),
            buffers: &[Some(Vertex::LAYOUT), Some(MeshInstance::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: surface_module,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample,
        multiview_mask: None,
        cache: None,
    })
}

fn line_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
    multisample: wgpu::MultisampleState,
) -> wgpu::RenderPipeline {
    let (surface_pipeline_layout, line_module) = (&layouts.surface, &modules.line);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz lines"),
        layout: Some(surface_pipeline_layout),
        vertex: wgpu::VertexState {
            module: line_module,
            entry_point: Some("vertex_main"),
            buffers: &[Some(LineVertex::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: line_module,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                // Alpha, not additive: additive ink is invisible on a light page.
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample,
        multiview_mask: None,
        cache: None,
    })
}

fn beam_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
    multisample: wgpu::MultisampleState,
) -> wgpu::RenderPipeline {
    let (beam_pipeline_layout, beam_module) = (&layouts.beam, &modules.beam);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz beams"),
        layout: Some(beam_pipeline_layout),
        vertex: wgpu::VertexState {
            module: beam_module,
            entry_point: Some("vertex_main"),
            buffers: &[Some(Vertex::LAYOUT), Some(BeamInstance::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: beam_module,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: Some(BEAM_BLEND),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            // The closed cone is wound inward: its visible faces are the ray's exit surface.
            // One exit shades the complete analytic segment, even with the camera inside.
            // Entry+exit shading used to integrate every outside-camera shaft twice.
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: None,
        multisample,
        multiview_mask: None,
        cache: None,
    })
}

fn laser_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
    multisample: wgpu::MultisampleState,
) -> wgpu::RenderPipeline {
    let (laser_pipeline_layout, laser_module) = (&layouts.laser, &modules.laser);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz lasers"),
        layout: Some(laser_pipeline_layout),
        vertex: wgpu::VertexState {
            module: laser_module,
            entry_point: Some("vertex_main"),
            // Six vertices generated per instance; the run itself is the only input.
            buffers: &[Some(LaserInstance::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: laser_module,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                // Added, not blended: where a scan path crosses itself it is brighter, which
                // is what a real one does and what makes a dense figure read as solid.
                blend: Some(additive_blend()),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample,
        multiview_mask: None,
        cache: None,
    })
}

fn shadow_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
) -> wgpu::RenderPipeline {
    let (shadow_pipeline_layout, shadow_module) = (&layouts.shadow, &modules.shadow);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz shadow"),
        layout: Some(shadow_pipeline_layout),
        vertex: wgpu::VertexState {
            module: shadow_module,
            entry_point: Some("vertex_main"),
            buffers: &[Some(Vertex::LAYOUT), Some(MeshInstance::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            // Drawing back faces into the map keeps the acne off the lit surfaces.
            cull_mode: Some(wgpu::Face::Front),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn cull_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    modules: &Modules,
    draft_mode: bool,
    entry: &str,
) -> wgpu::ComputePipeline {
    let (cull_pipeline_layout, cull_module) = (&layouts.cull, &modules.cull);

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("viz cull"),
        layout: Some(cull_pipeline_layout),
        module: cull_module,
        entry_point: Some(entry),
        compilation_options: wgpu::PipelineCompilationOptions {
            constants: &[("DRAFT_CONE_BOUNDS", if draft_mode { 1.0 } else { 0.0 })],
            ..Default::default()
        },
        cache: None,
    })
}

pub(super) fn overlay_pipeline(
    device: &wgpu::Device,
    overlay_pipeline_layout: &wgpu::PipelineLayout,
    overlay_module: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let format = surface_format;

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viz overlay"),
        layout: Some(overlay_pipeline_layout),
        vertex: wgpu::VertexState {
            module: overlay_module,
            entry_point: Some("vertex_main"),
            buffers: &[Some(crate::overlay::OverlayQuad::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: overlay_module,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn additive_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::REPLACE,
    }
}

fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("fullscreen"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{BEAM_BLEND, BEAM_WGSL};

    #[test]
    fn overlapping_haze_is_bounded_and_composited_as_premultiplied_light() {
        assert!(BEAM_WGSL.contains("1.0 - exp(-scatter)"));
        assert!(BEAM_WGSL.contains("input.colour.rgb * opacity, opacity"));
        assert_eq!(
            BEAM_BLEND.color.src_factor,
            wgpu::BlendFactor::One,
            "premultiplied beam colour must not be multiplied by alpha twice"
        );
        assert_eq!(
            BEAM_BLEND.color.dst_factor,
            wgpu::BlendFactor::OneMinusSrcAlpha,
            "each beam must converge instead of adding unbounded energy"
        );
    }
}

#[cfg(test)]
pub(in crate::renderer) fn test_cone_cull_pipeline(
    device: &wgpu::Device,
    bindings: &wgpu::BindGroupLayout,
    bounds: &wgpu::BindGroupLayout,
    enabled: bool,
) -> wgpu::ComputePipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Draft cull comparison layout"),
        bind_group_layouts: &[Some(bindings), Some(bounds)],
        immediate_size: 0,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Draft cull comparison shader"),
        source: wgpu::ShaderSource::Wgsl(CULL_WGSL.into()),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Draft cull comparison"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cull_main"),
        compilation_options: wgpu::PipelineCompilationOptions {
            constants: &[
                ("DRAFT_CONE_BOUNDS", if enabled { 1.0 } else { 0.0 }),
                ("PRECOMPUTED_BOUNDS", 0.0),
            ],
            ..Default::default()
        },
        cache: None,
    })
}
