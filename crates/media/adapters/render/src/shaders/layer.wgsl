// One media layer, composited over whatever is already in the program target.
//
// The vertex stage performs exactly the arithmetic `media_domain::geometry` performs on the CPU:
// scale the unit quad by the layer's size, rotate it around its own centre, translate it to that
// centre, then convert output pixels into clip space. Keeping both sides on the same formula is
// what lets a CPU-side corner assertion stand in for a rendered pixel.

struct Layer {
    // Centre of the quad, in output pixels.
    center: vec2<f32>,
    // Unrotated width and height, in output pixels.
    size: vec2<f32>,
    // Cosine and sine of the layer's rotation.
    rotation: vec2<f32>,
    // The output's pixel dimensions.
    output: vec2<f32>,
    // Layer tint in rgb; layer dimmer in a.
    tint: vec4<f32>,
    // x: grayscale amount. y, z, w: reserved for the mask and effect slices.
    controls: vec4<f32>,
};

@group(0) @binding(0) var<uniform> layer: Layer;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Two triangles, generated rather than fed from a buffer: the quad is the same every frame.
const CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(-0.5, -0.5),
    vec2<f32>( 0.5, -0.5),
    vec2<f32>( 0.5,  0.5),
    vec2<f32>(-0.5, -0.5),
    vec2<f32>( 0.5,  0.5),
    vec2<f32>(-0.5,  0.5),
);

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = CORNERS[index];

    let scaled = corner * layer.size;
    let cos_rotation = layer.rotation.x;
    let sin_rotation = layer.rotation.y;
    let rotated = vec2<f32>(
        scaled.x * cos_rotation - scaled.y * sin_rotation,
        scaled.x * sin_rotation + scaled.y * cos_rotation,
    );
    let pixel = layer.center + rotated;

    // Output pixels have their origin at the top left with y increasing downward; clip space has
    // its origin at the centre with y increasing upward.
    let clip = vec2<f32>(
        2.0 * pixel.x / layer.output.x - 1.0,
        1.0 - 2.0 * pixel.y / layer.output.y,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip, 0.0, 1.0);
    out.uv = corner + vec2<f32>(0.5, 0.5);
    return out;
}

// The weights the legacy renderer used, kept so a migrated show's grayscale looks the same.
const LUMINANCE = vec3<f32>(0.299, 0.587, 0.114);

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(source, source_sampler, in.uv);

    let gray = dot(sampled.rgb, LUMINANCE);
    let desaturated = mix(sampled.rgb, vec3<f32>(gray, gray, gray), layer.controls.x);
    let tinted = desaturated * layer.tint.rgb;

    // Layer dimmer becomes the alpha of the layer tint, so a dimmed layer reveals what is beneath
    // it rather than turning black.
    return vec4<f32>(tinted, sampled.a * layer.tint.a);
}
