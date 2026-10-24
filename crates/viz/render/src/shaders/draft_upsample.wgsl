// Full-resolution premultiplied composition; geometry depth rejects cross-edge beam samples.
@group(0) @binding(0) var beam_colour: texture_2d<f32>;
@group(0) @binding(1) var geometry_depth: texture_depth_2d;
struct Settings { near: f32, far: f32, pad0: f32, pad1: f32 };
@group(0) @binding(2) var<uniform> settings: Settings;
@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}
fn linear(depth: f32) -> f32 {
    return settings.near * settings.far / max(settings.far - depth * (settings.far - settings.near), 1e-6);
}
fn upsample_beam(pixel: vec4<f32>) -> vec4<f32> {
    let full_size = vec2<i32>(textureDimensions(geometry_depth));
    let half_size = vec2<i32>(textureDimensions(beam_colour));
    let uv = pixel.xy / vec2<f32>(full_size);
    let at = uv * vec2<f32>(half_size) - 0.5;
    let base = vec2<i32>(floor(at));
    let fraction = fract(at);
    let depth = linear(textureLoad(geometry_depth, clamp(vec2<i32>(pixel.xy), vec2<i32>(0), full_size - 1), 0));
    var colour = vec4<f32>(0.0);
    var weight_sum = 0.0;
    for (var y = 0; y < 2; y++) { for (var x = 0; x < 2; x++) {
        let coordinate = clamp(base + vec2<i32>(x, y), vec2<i32>(0), half_size - 1);
        let reference = vec2<i32>((vec2<f32>(coordinate) + 0.5) / vec2<f32>(half_size) * vec2<f32>(full_size));
        let delta = abs(linear(textureLoad(geometry_depth, clamp(reference, vec2<i32>(0), full_size - 1), 0)) - depth);
        let sample_colour = textureLoad(beam_colour, coordinate, 0);
        let bilinear = select(1.0 - fraction.x, fraction.x, x == 1) * select(1.0 - fraction.y, fraction.y, y == 1);
        let weight = bilinear * exp(-delta / max(0.01, depth * 0.02));
        colour += sample_colour * weight;
        weight_sum += weight;
    } }
    // A thin foreground edge may have no matching low-resolution sample. Reusing background
    // shafts would leak through opaque scenery; leave that unsupported pixel transparent.
    if (weight_sum <= 1e-6) { return vec4<f32>(0.0); }
    return colour / weight_sum;
}

@fragment fn fragment_main(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    return upsample_beam(pixel);
}
