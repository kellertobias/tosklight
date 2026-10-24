// Concatenated after draft_upsample: exactly the same depth-aware reconstruction.
@group(1) @binding(0) var opaque_hdr: texture_2d<f32>;

fn aces(colour: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((colour * (a * colour + b)) / (colour * (c * colour + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}
@fragment fn composite_main(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let beam = upsample_beam(pixel);
    let opaque = textureLoad(opaque_hdr, vec2<i32>(pixel.xy), 0).rgb;
    // Equivalent premultiplied source-over, without the intermediate Rgba16Float roundtrip.
    let colour = (beam.rgb + opaque * (1.0 - beam.a)) * settings.pad0;
    return vec4<f32>(aces(colour), 1.0);
}
