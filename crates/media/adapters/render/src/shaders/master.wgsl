// The master pass: the finished composite, tinted, dimmed, and flipped onto the output.

struct Master {
    // Master tint in rgb; master dimmer in a.
    tint: vec4<f32>,
    // xy: flip signs. z: master mask enabled. w: negative for an alpha-preserving layer preview.
    flip_mask: vec4<f32>,
    // xy: centre displacement in half-output units. zw: per-axis scale.
    transform: vec4<f32>,
    // xy: cosine/sine of master rotation. zw: cosine/sine of whole-shaper rotation.
    rotation: vec4<f32>,
    // xy: master-mask position in half-output units.
    mask_transform: vec4<f32>,
    // Left, right, top and bottom inward edge positions, normalized to the master rectangle.
    shaper_edges: vec4<f32>,
    // Tangents of the corresponding edge rotations.
    shaper_edge_tangents: vec4<f32>,
    // The slice of the canvas this screen shows. xy: its start, zw: its extent, both as canvas
    // fractions. The whole canvas is (0, 0, 1, 1).
    region: vec4<f32>,
    // xy: cosine/sine of the quarter-turn applied to this screen alone.
    region_rotation: vec4<f32>,
};

@group(0) @binding(0) var<uniform> master: Master;
@group(0) @binding(1) var program: texture_2d<f32>;
@group(0) @binding(2) var program_sampler: sampler;
@group(0) @binding(3) var mask: texture_2d<f32>;

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
    let scaled = corner * master.transform.zw;
    let rotated = vec2<f32>(
        scaled.x * master.rotation.x - scaled.y * master.rotation.y,
        scaled.x * master.rotation.y + scaled.y * master.rotation.x,
    );
    out.clip_position = vec4<f32>(rotated + master.transform.xy, 0.0, 1.0);
    // Flipping the sampled coordinate rather than the geometry keeps the pass a single full-screen
    // triangle pair whichever way the operator has turned the output.
    let uv = vec2<f32>(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
    let flipped = vec2<f32>(
        select(uv.x, 1.0 - uv.x, master.flip_mask.x < 0.0),
        select(uv.y, 1.0 - uv.y, master.flip_mask.y < 0.0),
    );
    // The screen shows one slice of the canvas, turned to suit how it is hung. Turning the sampled
    // coordinate rather than the canvas leaves every other screen reading the picture as authored.
    let centred = flipped - vec2<f32>(0.5);
    let turned = vec2<f32>(
        centred.x * master.region_rotation.x + centred.y * master.region_rotation.y,
        -centred.x * master.region_rotation.y + centred.y * master.region_rotation.x,
    ) + vec2<f32>(0.5);
    out.uv = master.region.xy + turned * master.region.zw;
    return out;
}

const LUMINANCE = vec3<f32>(0.299, 0.587, 0.114);

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let centred = in.uv - vec2<f32>(0.5);
    let shaped = vec2<f32>(
        centred.x * master.rotation.z + centred.y * master.rotation.w,
        -centred.x * master.rotation.w + centred.y * master.rotation.z,
    ) + vec2<f32>(0.5);
    let left = master.shaper_edges.x + master.shaper_edge_tangents.x * (shaped.y - 0.5);
    let right = 1.0 - master.shaper_edges.y + master.shaper_edge_tangents.y * (shaped.y - 0.5);
    let top = master.shaper_edges.z + master.shaper_edge_tangents.z * (shaped.x - 0.5);
    let bottom = 1.0 - master.shaper_edges.w + master.shaper_edge_tangents.w * (shaped.x - 0.5);
    if shaped.x < left || shaped.x > right || shaped.y < top || shaped.y > bottom {
        discard;
    }
    let sampled = textureSample(program, program_sampler, in.uv);

    // The output-level mask applies to the finished composite, after every layer has landed, so
    // it shapes the whole image rather than one layer's contribution to it.
    var strength = 1.0;
    if master.flip_mask.z > 0.0 {
        let mask_uv = in.uv - master.mask_transform.xy * 0.5;
        var value = 0.0;
        if mask_uv.x >= 0.0 && mask_uv.x <= 1.0 && mask_uv.y >= 0.0 && mask_uv.y <= 1.0 {
            let read = textureSample(mask, program_sampler, mask_uv);
            value = dot(read.rgb, LUMINANCE);
        }
        strength = mix(1.0, clamp(value, 0.0, 1.0), master.flip_mask.z);
    }

    // Master dimmer is the final black overlay: scaling the composite is the same result with one
    // fewer pass.
    let contribution = master.tint.a * strength;
    let output_alpha = select(1.0, sampled.a * contribution, master.flip_mask.w < 0.0);
    return vec4<f32>(sampled.rgb * master.tint.rgb * contribution, output_alpha);
}
