// A layer's finished look wrapped onto a 3D model.
//
// The look texture holds exactly what the flat layer would draw — source, effects, tint,
// greyscale, mask and dimmer — rendered with normal alpha blending over transparent black, so its
// colour is premultiplied. This pass writes straight colour, because the mapped image is then
// composited by the ordinary layer shader, which expects straight colour like any other source.

struct Mesh {
    model_view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> mesh: Mesh;
@group(0) @binding(1) var look: texture_2d<f32>;
@group(0) @binding(2) var look_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = mesh.model_view_projection * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let colour = textureSample(look, look_sampler, in.uv);
    // A fully transparent texel must not occlude the model behind it.
    if colour.a <= 0.5 / 255.0 {
        discard;
    }
    return vec4<f32>(colour.rgb / colour.a, colour.a);
}
