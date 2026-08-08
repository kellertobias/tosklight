// The master pass: the finished composite, tinted, dimmed, and flipped onto the output.

struct Master {
    // Master tint in rgb; master dimmer in a.
    tint: vec4<f32>,
    // Per-axis sign: -1 flips that axis, 1 leaves it alone.
    flip: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> master: Master;
@group(0) @binding(1) var program: texture_2d<f32>;
@group(0) @binding(2) var program_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

const CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
);

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = CORNERS[index];

    var out: VertexOutput;
    out.clip_position = vec4<f32>(corner, 0.0, 1.0);
    // Flipping the sampled coordinate rather than the geometry keeps the pass a single full-screen
    // triangle pair whichever way the operator has turned the output.
    let uv = vec2<f32>(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
    out.uv = vec2<f32>(
        select(uv.x, 1.0 - uv.x, master.flip.x < 0.0),
        select(uv.y, 1.0 - uv.y, master.flip.y < 0.0),
    );
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(program, program_sampler, in.uv);
    // Master dimmer is the final black overlay: scaling the composite is the same result with one
    // fewer pass.
    return vec4<f32>(sampled.rgb * master.tint.rgb * master.tint.a, 1.0);
}
