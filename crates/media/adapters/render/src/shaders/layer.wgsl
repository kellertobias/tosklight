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
    // x: grayscale amount. y, z, w: reserved for the effect slice.
    controls: vec4<f32>,
    blur: vec4<f32>,
    // x, y: the mask's own scale about the layer centre. z: 1 when inverted. w: mask opacity,
    // which is zero when the layer has no mask at all.
    mask: vec4<f32>,
    // x: 1 when the mask reads alpha rather than luminance. yz: mask position in half-layer units.
    mask_source: vec4<f32>,
    // 0: none/unsupported, 1: Analog TV, 2: Digital TV, 3: Blur, 4: Kaleidoscope,
    // 5: Rasterized Print, 6: Beat Scan, 7: Beat Grid Wave, 8: Beat Form Flash,
    // 9: Drawn Image.
    effect_types: vec4<u32>,
    effect_mixes: vec4<f32>,
    // Typed normalized parameters for slots 1..4. Analog TV is curvature, distortion, grain,
    // glitching. Future effect types interpret their own row through the advertised contract.
    effect_parameters: array<vec4<f32>, 4>,
    // Fifth typed parameter, indexed by slot.
    effect_parameter_tail: vec4<f32>,
    effect_seeds: vec4<f32>,
    // x: authoritative playback seconds. y/z: output dimensions.
    effect_clock: vec4<f32>,
    beat_scan_positions: array<vec4<f32>, 16>,
    beat_scan_counts: array<vec4<f32>, 16>,
    beat_event_x: array<vec4<f32>, 16>,
    beat_event_y: array<vec4<f32>, 16>,
};

@group(0) @binding(0) var<uniform> layer: Layer;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;
@group(0) @binding(3) var mask: texture_2d<f32>;

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

fn hash21(value: vec2<f32>) -> f32 {
    return fract(sin(dot(value, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

struct EffectCoordinates {
    uv: vec2<f32>,
    validity: f32,
};

/// Source-coordinate part of Analog TV. It is applied before the source sample so effect slots
/// compose in order, and returns an explicit validity instead of relying on the clamp-to-edge
/// sampler (which would repeat the outside edge around a curved CRT).
fn analog_coordinates(
    input: EffectCoordinates,
    parameters: vec4<f32>,
    mix_amount: f32,
    seed: f32,
) -> EffectCoordinates {
    let curvature = parameters.x * mix_amount;
    let distortion = parameters.y * mix_amount;
    let glitching = parameters.w * mix_amount;
    if curvature <= 0.0 && distortion <= 0.0 && glitching <= 0.0 {
        return input;
    }
    let seconds = layer.effect_clock.x;

    var centred = input.uv * 2.0 - vec2<f32>(1.0);
    let radius_squared = dot(centred, centred);
    // Pull the corners inward before bending them back out. This controlled overscan keeps normal
    // content filling the rounded face while still allowing the extreme curved corners to fade.
    centred *= (1.0 - curvature * 0.075) * (1.0 + curvature * 0.18 * radius_squared);
    var uv = centred * 0.5 + vec2<f32>(0.5);

    // Continuous horizontal hold and line displacement. Every term is smooth in time and y, so
    // this can never turn into the grid-aligned macroblocks owned by Digital TV.
    let line = floor(uv.y * max(layer.effect_clock.z, 1.0));
    let slow_sync = sin(uv.y * 31.0 + seconds * 3.7 + seed * 19.0);
    let line_jitter = hash21(vec2<f32>(line, floor(seconds * 24.0) + seed * 991.0)) - 0.5;
    uv.x += distortion * (slow_sync * 0.012 + line_jitter * 0.010);

    // Intermittent failures are held for a few frames by the event bucket. A triggered horizontal
    // band tears, and the rare stronger event rolls vertically; neither creates rectangular tiles.
    let event_bucket = floor(seconds * 6.0);
    let event = hash21(vec2<f32>(event_bucket, seed * 4093.0));
    let triggered = select(0.0, 1.0, event > 1.0 - glitching * 0.42);
    let band_center = hash21(vec2<f32>(event_bucket + 17.0, seed * 1877.0));
    let band = 1.0 - smoothstep(0.035, 0.085, abs(uv.y - band_center));
    uv.x += triggered * band * (hash21(vec2<f32>(event_bucket, seed)) - 0.5) * 0.20;
    let roll = triggered * select(0.0, 1.0, event > 0.985) * glitching;
    uv.y = mix(uv.y, fract(uv.y + seconds * 0.18 + seed), roll);

    let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
    var result: EffectCoordinates;
    result.uv = uv;
    result.validity = input.validity * smoothstep(-0.012, 0.012, edge);
    return result;
}

/// Color part of Analog TV, after the ordered UV transforms and source sample.
fn analog_colour(
    colour: vec4<f32>,
    uv: vec2<f32>,
    parameters: vec4<f32>,
    mix_amount: f32,
    seed: f32,
) -> vec4<f32> {
    let distortion = parameters.y * mix_amount;
    let grain = parameters.z * mix_amount;
    let glitching = parameters.w * mix_amount;
    if distortion <= 0.0 && grain <= 0.0 && glitching <= 0.0 {
        return colour;
    }
    let seconds = layer.effect_clock.x;

    // Bounded analog chroma misregistration: luma stays anchored while red and blue drift by less
    // than two percent of the image even at the endpoint.
    let chroma_shift = distortion * 0.012 * sin(uv.y * 19.0 + seconds * 2.3 + seed * 7.0);
    let red = textureSample(source, source_sampler, uv + vec2<f32>(chroma_shift, 0.0)).r;
    let blue = textureSample(source, source_sampler, uv - vec2<f32>(chroma_shift, 0.0)).b;
    var rgb = vec3<f32>(mix(colour.r, red, distortion), colour.g, mix(colour.b, blue, distortion));

    let pixel = floor(uv * layer.effect_clock.yz);
    let noise = hash21(pixel + vec2<f32>(floor(seconds * 30.0), seed * 8191.0)) - 0.5;
    let scanline = sin(uv.y * layer.effect_clock.z * 3.14159265);
    rgb += noise * grain * 0.22;
    rgb *= 1.0 - grain * (0.055 + 0.035 * scanline);

    let event_bucket = floor(seconds * 6.0);
    let event = hash21(vec2<f32>(event_bucket, seed * 4093.0));
    let triggered = select(0.0, 1.0, event > 1.0 - glitching * 0.42);
    let disturbance = (hash21(vec2<f32>(event_bucket + 41.0, seed)) - 0.5) * 0.35;
    rgb *= 1.0 + triggered * disturbance;
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), colour.a);
}

fn digital_grid(parameters: vec4<f32>) -> f32 {
    // Normalized grid density keeps the artifact scale independent of output resolution.
    return mix(64.0, 7.0, parameters.y);
}

/// Grid-aligned tile displacement for damaged compressed streams. Event buckets hold the same
/// wrong tile for a quarter second instead of generating unrelated full-screen noise each frame.
fn digital_coordinates(
    input: EffectCoordinates,
    parameters: vec4<f32>,
    glitching_parameter: f32,
    mix_amount: f32,
    seed: f32,
) -> EffectCoordinates {
    let displacement = parameters.z * mix_amount;
    let glitching = glitching_parameter * mix_amount;
    if displacement <= 0.0 && glitching <= 0.0 {
        return input;
    }
    let cells = digital_grid(parameters);
    let tile = floor(input.uv * cells);
    let held_event = floor(layer.effect_clock.x * 4.0);
    let choice = hash21(tile + vec2<f32>(held_event * 13.0, seed * 8191.0));
    let displaced = select(0.0, 1.0, choice > 1.0 - displacement * 0.58);
    let stream_failure = select(0.0, 1.0, choice > 1.0 - glitching * 0.36);
    let offset_cell = vec2<f32>(
        floor((hash21(tile.yx + vec2<f32>(held_event, seed * 97.0)) - 0.5) * 7.0),
        floor((hash21(tile + vec2<f32>(seed * 193.0, held_event)) - 0.5) * 5.0),
    );
    let amount = max(displaced * displacement, stream_failure * glitching);
    var result = input;
    // Clamp is the documented source-boundary contract: corruption never samples outside source.
    result.uv = clamp(input.uv + offset_cell / cells * amount, vec2<f32>(0.0), vec2<f32>(1.0));
    return result;
}

/// Compression/chroma corruption operates on the previous slot's color and stays rectangular.
/// There is deliberately no scanline or per-pixel snow term: those belong to Analog TV.
fn digital_colour(
    colour: vec4<f32>,
    uv: vec2<f32>,
    parameters: vec4<f32>,
    glitching_parameter: f32,
    mix_amount: f32,
    seed: f32,
) -> vec4<f32> {
    let compression = parameters.x * mix_amount;
    let chroma_damage = parameters.w * mix_amount;
    let glitching = glitching_parameter * mix_amount;
    if compression <= 0.0 && chroma_damage <= 0.0 && glitching <= 0.0 {
        return colour;
    }

    let cells = digital_grid(parameters);
    let tile = floor(uv * cells);
    let held_event = floor(layer.effect_clock.x * 4.0);
    let tile_hash = hash21(tile + vec2<f32>(held_event * 17.0, seed * 4093.0));
    var rgb = colour.rgb;

    // Posterization and tile-edge ringing approximate damaged transform coefficients.
    let levels = mix(255.0, 6.0, compression);
    let quantized = round(rgb * levels) / levels;
    let within_tile = fract(uv * cells);
    let edge = 1.0 - smoothstep(0.0, 0.16, min(min(within_tile.x, within_tile.y), min(1.0 - within_tile.x, 1.0 - within_tile.y)));
    rgb = mix(rgb, quantized + (tile_hash - 0.5) * edge * 0.14, compression);

    // Damage chroma by rectangular block while leaving luma comparatively stable.
    let luma = dot(rgb, LUMINANCE);
    let chroma = rgb - vec3<f32>(luma);
    let chroma_hold = select(0.35, 1.0, tile_hash > 0.72);
    let damaged_chroma = chroma * (1.0 - chroma_damage * chroma_hold);
    let channel_bias = vec3<f32>(tile_hash - 0.5, 0.25 - tile_hash * 0.5, 0.5 - tile_hash) * 0.22;
    rgb = vec3<f32>(luma) + damaged_chroma + channel_bias * chroma_damage;

    // A held subset of macroblocks loses a channel or brightness during a stream failure.
    let corrupted = select(0.0, 1.0, tile_hash > 1.0 - glitching * 0.48);
    let failure = vec3<f32>(rgb.g * 0.25, rgb.b, rgb.r * 0.1);
    rgb = mix(rgb, failure * (0.45 + tile_hash * 0.75), corrupted * glitching);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), colour.a);
}

/// How much of the layer this pixel's mask lets through.
///
/// The mask carries its own scale about the layer centre rather than inheriting the layer's, so an
/// operator can hold a mask still while the media moves behind it. Outside the mask's own extent
/// there is nothing to read, and an unread mask hides rather than reveals — otherwise scaling a
/// mask down would flood the output with unmasked layer.
fn mask_strength(uv: vec2<f32>) -> f32 {
    let opacity = layer.mask.w;
    if opacity <= 0.0 {
        return 1.0;
    }

    let scale = max(layer.mask.xy, vec2<f32>(0.0001));
    let offset = layer.mask_source.yz * 0.5;
    let centred = (uv - vec2<f32>(0.5) - offset) / scale + vec2<f32>(0.5);
    var strength = 0.0;
    if centred.x >= 0.0 && centred.x <= 1.0 && centred.y >= 0.0 && centred.y <= 1.0 {
        let sampled = textureSample(mask, source_sampler, centred);
        strength = select(dot(sampled.rgb, LUMINANCE), sampled.a, layer.mask_source.x > 0.5);
    }
    if layer.mask.z > 0.5 {
        strength = 1.0 - strength;
    }
    // Opacity blends between the unmasked layer and the masked result, so a mask can be eased in.
    return mix(1.0, clamp(strength, 0.0, 1.0), opacity);
}

fn blurred_source(uv: vec2<f32>, amount: f32, effect_mix: f32) -> vec4<f32> {
    let original = textureSample(source, source_sampler, uv);
    if amount <= 0.0 || effect_mix <= 0.0 {
        return original;
    }
    let dimensions = vec2<f32>(textureDimensions(source));
    let radius = amount * 18.0 / max(dimensions, vec2<f32>(1.0));
    var blurred = original * 0.2;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>( radius.x, 0.0)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>(-radius.x, 0.0)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>(0.0,  radius.y)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>(0.0, -radius.y)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>( radius.x,  radius.y)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>(-radius.x,  radius.y)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>( radius.x, -radius.y)) * 0.1;
    blurred += textureSample(source, source_sampler, uv + vec2<f32>(-radius.x, -radius.y)) * 0.1;
    return mix(original, blurred, effect_mix);
}

fn kaleidoscope_coordinates(
    coordinates: EffectCoordinates,
    repetitions: f32,
    angle_degrees: f32,
    effect_mix: f32,
) -> EffectCoordinates {
    var result = coordinates;
    let count = clamp(round(repetitions), 1.0, 16.0);
    if count <= 1.0 || effect_mix <= 0.0 {
        return result;
    }

    let centred = coordinates.uv - vec2<f32>(0.5);
    let radius = length(centred);
    let axis = radians(clamp(angle_degrees, -180.0, 180.0));
    let sector = 6.28318530718 / count;
    let source_angle = atan2(centred.y, centred.x) - axis;
    let wrapped = fract(source_angle / sector + 0.5) * sector - sector * 0.5;
    let mirrored_angle = abs(wrapped) + axis;
    let mirrored = vec2<f32>(0.5) + radius * vec2<f32>(cos(mirrored_angle), sin(mirrored_angle));
    result.uv = mix(coordinates.uv, clamp(mirrored, vec2<f32>(0.0), vec2<f32>(1.0)), effect_mix);
    return result;
}

fn print_dot(channel: f32, grid: vec2<f32>, phase: vec2<f32>) -> f32 {
    let point = fract(grid + phase) - vec2<f32>(0.5);
    let radius = sqrt(clamp(channel, 0.0, 1.0)) * 0.48;
    return 1.0 - smoothstep(radius - 0.06, radius + 0.06, length(point));
}

fn rasterized_source(
    uv: vec2<f32>,
    mode: f32,
    dot_size: f32,
    effect_mix: f32,
) -> vec4<f32> {
    let original = textureSample(source, source_sampler, uv);
    if effect_mix <= 0.0 {
        return original;
    }

    let dimensions = max(vec2<f32>(textureDimensions(source)), vec2<f32>(1.0));
    let cell_size = clamp(dot_size, 2.0, 32.0);
    let pixels = uv * dimensions;
    let grid = pixels / cell_size;
    let centre_uv = clamp(
        (floor(grid) + vec2<f32>(0.5)) * cell_size / dimensions,
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    let colour = textureSample(source, source_sampler, centre_uv);
    var printed: vec3<f32>;
    var ink_coverage: f32;
    if mode < 0.5 {
        let darkness = 1.0 - dot(colour.rgb, LUMINANCE);
        let ink = print_dot(darkness, grid, vec2<f32>(0.0));
        printed = vec3<f32>(1.0 - ink);
        ink_coverage = ink;
    } else {
        let cmy = vec3<f32>(1.0) - colour.rgb;
        let black = min(cmy.r, min(cmy.g, cmy.b));
        let cyan = print_dot(cmy.r - black, grid, vec2<f32>(0.00, 0.00));
        let magenta = print_dot(cmy.g - black, grid, vec2<f32>(0.31, 0.17));
        let yellow = print_dot(cmy.b - black, grid, vec2<f32>(0.13, 0.37));
        let key = print_dot(black, grid, vec2<f32>(0.41, 0.43));
        printed = vec3<f32>(
            (1.0 - cyan) * (1.0 - key),
            (1.0 - magenta) * (1.0 - key),
            (1.0 - yellow) * (1.0 - key),
        );
        ink_coverage = 1.0 - min(printed.r, min(printed.g, printed.b));
    }
    // Paper is transparent: only the printed dots replace the layer beneath. Preserve the
    // source alpha on ink so a transparent source cannot manufacture opaque print.
    return vec4<f32>(
        mix(original.rgb, printed, effect_mix),
        mix(original.a, original.a * ink_coverage, effect_mix),
    );
}

fn beat_scan_source(
    original: vec4<f32>,
    uv: vec2<f32>,
    parameters: vec4<f32>,
    effect_mix: f32,
    slot: u32,
) -> vec4<f32> {
    if effect_mix <= 0.0 {
        return original;
    }
    let width = clamp(parameters.x, 0.01, 0.25);
    let soft = parameters.y >= 0.5;
    let falloff = width * clamp(parameters.z, 0.0, 1.0);
    var line_strength = 0.0;
    for (var event = 0u; event < 16u; event += 1u) {
        let base = layer.beat_scan_positions[event][slot];
        let count = u32(clamp(round(layer.beat_scan_counts[event][slot]), 0.0, 3.0));
        for (var line = 0u; line < 3u; line += 1u) {
            if line < count {
                let centre = base + f32(line) * width * 1.4;
                let distance = abs(uv.y - centre);
                let half_width = width * 0.5;
                var strength = select(0.0, 1.0, distance <= half_width);
                if soft {
                    strength = 1.0 - smoothstep(
                        half_width,
                        half_width + max(falloff, 0.0001),
                        distance,
                    );
                }
                line_strength = max(line_strength, strength);
            }
        }
    }
    let scanned = mix(original.rgb, vec3<f32>(1.0), line_strength * 0.72);
    return vec4<f32>(mix(original.rgb, scanned, effect_mix), original.a);
}

fn hue_colour(degrees: f32) -> vec3<f32> {
    let hue = fract(degrees / 360.0);
    let shifted = abs(fract(hue + vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    return clamp(shifted - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn beat_grid_wave_source(
    original: vec4<f32>,
    uv: vec2<f32>,
    parameters: vec4<f32>,
    hue: f32,
    brightness: f32,
    effect_mix: f32,
    slot: u32,
) -> vec4<f32> {
    if effect_mix <= 0.0 {
        return original;
    }
    let density = clamp(parameters.x, 6.0, 64.0);
    let wave_height = clamp(parameters.y, 0.0, 1.0);
    let origin = u32(clamp(round(parameters.w), 0.0, 4.0));
    var distance_from_origin = length((uv - vec2<f32>(0.5)) * vec2<f32>(1.35, 1.0)) / 0.84;
    if origin == 1u {
        distance_from_origin = uv.y;
    } else if origin == 2u {
        distance_from_origin = 1.0 - uv.x;
    } else if origin == 3u {
        distance_from_origin = 1.0 - uv.y;
    } else if origin == 4u {
        distance_from_origin = uv.x;
    }

    var displacement = 0.0;
    var crest = 0.0;
    for (var event = 0u; event < 16u; event += 1u) {
        let progress = layer.beat_scan_positions[event][slot];
        if progress >= 0.0 && progress <= 1.0 {
            let strength = clamp(layer.beat_scan_counts[event][slot], 0.0, 1.0);
            let distance = abs(distance_from_origin - progress * 1.18);
            let pulse = 1.0 - smoothstep(0.015, 0.12, distance);
            displacement += pulse * wave_height * strength;
            crest = max(crest, pulse * strength);
        }
    }

    // A reciprocal-depth plane makes the horizontal lines bunch toward the horizon while the
    // vertical lines converge, giving the generated grid visible three-dimensional perspective.
    let depth = clamp(uv.y - 0.08 + displacement * 0.075, 0.025, 0.92);
    let plane_y = 1.0 / depth;
    let plane_x = (uv.x - 0.5) / depth;
    let grid_coordinates = vec2<f32>(plane_x * density * 0.16, plane_y * density * 0.055);
    let cell = abs(fract(grid_coordinates) - vec2<f32>(0.5));
    let derivatives = max(fwidth(grid_coordinates), vec2<f32>(0.001));
    let vertical = 1.0 - smoothstep(0.45 - derivatives.x, 0.5, cell.x);
    let horizontal = 1.0 - smoothstep(0.45 - derivatives.y, 0.5, cell.y);
    let line = clamp(max(vertical, horizontal), 0.0, 1.0);
    let horizon = smoothstep(0.08, 0.22, uv.y);
    let colour = hue_colour(hue) * clamp(brightness, 0.1, 2.0);
    let grid = colour * (line * (0.42 + depth * 0.58) + crest * 0.8) * horizon;
    let scene = vec3<f32>(0.003, 0.005, 0.012) + grid;
    return vec4<f32>(mix(original.rgb, scene, effect_mix), original.a);
}

fn beat_form_flash_source(
    original: vec4<f32>,
    uv: vec2<f32>,
    parameters: vec4<f32>,
    effect_mix: f32,
    slot: u32,
) -> vec4<f32> {
    if effect_mix <= 0.0 {
        return original;
    }
    var colour = vec3<f32>(0.0);
    var alpha = 0.0;
    for (var event = 0u; event < 16u; event += 1u) {
        let progress = layer.beat_scan_positions[event][slot];
        if progress >= 0.0 && progress <= 1.0 {
            let centre = vec2<f32>(layer.beat_event_x[event][slot], layer.beat_event_y[event][slot]);
            let variation = clamp(layer.beat_scan_counts[event][slot], 0.25, 1.75);
            let start_scale = clamp(parameters.x, 1.0, 4.0) * variation;
            let scale = mix(start_scale, 0.18 * variation, smoothstep(0.0, 1.0, progress));
            let source_uv = (uv - centre) / max(scale, 0.001) + vec2<f32>(0.5);
            if all(source_uv >= vec2<f32>(0.0)) && all(source_uv <= vec2<f32>(1.0)) {
                let form = textureSample(source, source_sampler, source_uv);
                let form_alpha = form.a * pow(1.0 - progress, 2.0);
                colour = colour + form.rgb * form_alpha * (1.0 - alpha);
                alpha = alpha + form_alpha * (1.0 - alpha);
            }
        }
    }
    let forms = vec4<f32>(select(vec3<f32>(0.0), colour / max(alpha, 0.0001), alpha > 0.0), alpha);
    return mix(original, forms, effect_mix);
}

fn drawn_image_source(uv: vec2<f32>, parameters: vec4<f32>, effect_mix: f32) -> vec4<f32> {
    let original = textureSample(source, source_sampler, uv);
    if effect_mix <= 0.0 { return original; }
    let dimensions = max(vec2<f32>(textureDimensions(source)), vec2<f32>(1.0));
    let detail = clamp(parameters.y, 0.0, 1.0);
    let pixel = (1.0 + (1.0 - detail) * 3.0) / dimensions;
    let left = dot(textureSample(source, source_sampler, clamp(uv - vec2<f32>(pixel.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb, LUMINANCE);
    let right = dot(textureSample(source, source_sampler, clamp(uv + vec2<f32>(pixel.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb, LUMINANCE);
    let top = dot(textureSample(source, source_sampler, clamp(uv - vec2<f32>(0.0, pixel.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb, LUMINANCE);
    let bottom = dot(textureSample(source, source_sampler, clamp(uv + vec2<f32>(0.0, pixel.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb, LUMINANCE);
    let edge = smoothstep(mix(0.22, 0.035, detail), mix(0.48, 0.12, detail), length(vec2<f32>(right - left, bottom - top)));
    let levels = mix(4.0, 10.0, detail);
    let illustrated = floor(original.rgb * levels + 0.5) / levels;
    let paper = mix(vec3<f32>(0.96, 0.94, 0.88), illustrated, 0.9) * (1.0 - edge * 0.82);
    let strength = clamp(parameters.x, 0.0, 1.0) * effect_mix;
    return vec4<f32>(mix(original.rgb, paper, strength), original.a);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var coordinates: EffectCoordinates;
    coordinates.uv = in.uv;
    coordinates.validity = 1.0;
    for (var slot = 0u; slot < 4u; slot += 1u) {
        if layer.effect_types[slot] == 1u {
            coordinates = analog_coordinates(
                coordinates,
                layer.effect_parameters[slot],
                layer.effect_mixes[slot],
                layer.effect_seeds[slot],
            );
        } else if layer.effect_types[slot] == 2u {
            coordinates = digital_coordinates(
                coordinates,
                layer.effect_parameters[slot],
                layer.effect_parameter_tail[slot],
                layer.effect_mixes[slot],
                layer.effect_seeds[slot],
            );
        } else if layer.effect_types[slot] == 4u {
            coordinates = kaleidoscope_coordinates(
                coordinates,
                layer.effect_parameters[slot].x,
                layer.effect_parameters[slot].y,
                layer.effect_mixes[slot],
            );
        }
    }

    var sampled = textureSample(source, source_sampler, coordinates.uv);
    for (var slot = 0u; slot < 4u; slot += 1u) {
        if layer.effect_types[slot] == 5u {
            sampled = rasterized_source(
                coordinates.uv,
                layer.effect_parameters[slot].x,
                layer.effect_parameters[slot].y,
                layer.effect_mixes[slot],
            );
        } else if layer.effect_types[slot] == 6u {
            sampled = beat_scan_source(
                sampled,
                coordinates.uv,
                layer.effect_parameters[slot],
                layer.effect_mixes[slot],
                slot,
            );
        } else if layer.effect_types[slot] == 7u {
            sampled = beat_grid_wave_source(
                sampled,
                coordinates.uv,
                layer.effect_parameters[slot],
                layer.effect_parameter_tail[slot],
                layer.effect_seeds[slot],
                layer.effect_mixes[slot],
                slot,
            );
        } else if layer.effect_types[slot] == 8u {
            sampled = beat_form_flash_source(
                sampled,
                coordinates.uv,
                layer.effect_parameters[slot],
                layer.effect_mixes[slot],
                slot,
            );
        } else if layer.effect_types[slot] == 9u {
            sampled = drawn_image_source(coordinates.uv, layer.effect_parameters[slot], layer.effect_mixes[slot]);
        }
    }
    sampled = blurred_source(coordinates.uv, layer.blur.x, 1.0);
    for (var slot = 0u; slot < 4u; slot += 1u) {
        if layer.effect_types[slot] == 1u {
            sampled = analog_colour(
                sampled,
                coordinates.uv,
                layer.effect_parameters[slot],
                layer.effect_mixes[slot],
                layer.effect_seeds[slot],
            );
        } else if layer.effect_types[slot] == 2u {
            sampled = digital_colour(
                sampled,
                coordinates.uv,
                layer.effect_parameters[slot],
                layer.effect_parameter_tail[slot],
                layer.effect_mixes[slot],
                layer.effect_seeds[slot],
            );
        }
    }
    sampled = vec4<f32>(sampled.rgb * coordinates.validity, sampled.a);

    let gray = dot(sampled.rgb, LUMINANCE);
    let desaturated = mix(sampled.rgb, vec3<f32>(gray, gray, gray), layer.controls.x);
    let tinted = desaturated * layer.tint.rgb;

    // Layer dimmer becomes the alpha of the layer tint, so a dimmed layer reveals what is beneath
    // it rather than turning black.
    return vec4<f32>(tinted, sampled.a * layer.tint.a * mask_strength(in.uv));
}
