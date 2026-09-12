// Shared by every generated visualizer.
//
// One uniform layout, one analysis texture, one fullscreen pass. A visualizer contributes only a
// `shade` function, so adding one cannot change how any other is bound, sampled, or timed.

struct Visualizer {
    // width, height, aspect, seconds
    resolution: vec4<f32>,
    // bass, mid, treble, energy
    audio0: vec4<f32>,
    // peak, beat (1 on the frame of a beat, decaying after), bpm, beat phase 0..1
    audio1: vec4<f32>,
    // kick, snare, hi-hat hits (1 when the instrument strikes, decaying after), spare
    audio2: vec4<f32>,
    // kick, snare, hi-hat levels, each auto-ranged 0..1, spare
    audio3: vec4<f32>,
    primary: vec4<f32>,
    secondary: vec4<f32>,
    // count, size, speed, amount
    params0: vec4<f32>,
    // radius, thickness, reactivity, decay
    params1: vec4<f32>,
    // zoom, iterations, threshold, smoothing
    params2: vec4<f32>,
    // gravity, lifetime, curvature, mode
    params3: vec4<f32>,
    // mirror, filled, wireframe, spare
    flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> visualizer: Visualizer;
// Row 0 is the 512-point waveform. Row 1 holds the 64 spectrum bands in its first texels, the 64
// held levels in the texels after them, and then the three accumulated tone turns.
@group(0) @binding(1) var analysis: texture_2d<f32>;

const WAVEFORM_POINTS: i32 = 512;
const BANDS: i32 = 64;
const TAU: f32 = 6.2831853;

fn seconds() -> f32 { return visualizer.resolution.w; }
fn aspect() -> f32 { return visualizer.resolution.z; }
fn bass() -> f32 { return visualizer.audio0.x; }
fn mid() -> f32 { return visualizer.audio0.y; }
fn treble() -> f32 { return visualizer.audio0.z; }
fn energy() -> f32 { return visualizer.audio0.w; }
fn peak() -> f32 { return visualizer.audio1.x; }
fn beat() -> f32 { return visualizer.audio1.y; }
fn beat_phase() -> f32 { return visualizer.audio1.w; }
fn kick() -> f32 { return visualizer.audio2.x; }
fn snare() -> f32 { return visualizer.audio2.y; }
fn hihat() -> f32 { return visualizer.audio2.z; }
fn kick_level() -> f32 { return visualizer.audio3.x; }
fn snare_level() -> f32 { return visualizer.audio3.y; }
fn hihat_level() -> f32 { return visualizer.audio3.z; }

fn count() -> f32 { return visualizer.params0.x; }
fn size() -> f32 { return visualizer.params0.y; }
fn speed() -> f32 { return visualizer.params0.z; }
fn amount() -> f32 { return visualizer.params0.w; }
fn radius() -> f32 { return visualizer.params1.x; }
fn thickness() -> f32 { return visualizer.params1.y; }
fn reactivity() -> f32 { return visualizer.params1.z; }
fn decay() -> f32 { return visualizer.params1.w; }
fn zoom() -> f32 { return visualizer.params2.x; }
fn iterations() -> f32 { return visualizer.params2.y; }
fn threshold() -> f32 { return visualizer.params2.z; }
fn smoothing() -> f32 { return visualizer.params2.w; }
fn gravity() -> f32 { return visualizer.params3.x; }
fn lifetime() -> f32 { return visualizer.params3.y; }
fn curvature() -> f32 { return visualizer.params3.z; }
fn mode() -> f32 { return visualizer.params3.w; }
fn mirrored() -> bool { return visualizer.flags.x > 0.5; }
fn filled() -> bool { return visualizer.flags.y > 0.5; }
fn wireframe() -> bool { return visualizer.flags.z > 0.5; }

fn primary() -> vec3<f32> { return visualizer.primary.rgb; }
fn secondary() -> vec3<f32> { return visualizer.secondary.rgb; }

/// One spectrum band, low frequency first. Out-of-range asks read as silence.
fn band(index: i32) -> f32 {
    if index < 0 || index >= BANDS { return 0.0; }
    return textureLoad(analysis, vec2<i32>(index, 1), 0).x;
}

/// The band at a normalized position, interpolated so a bar count above 64 still moves smoothly.
fn band_at(position: f32) -> f32 {
    let scaled = clamp(position, 0.0, 1.0) * f32(BANDS - 1);
    let low = i32(floor(scaled));
    return mix(band(low), band(low + 1), fract(scaled));
}

/// One band's held level, `0..1`, low frequency first.
///
/// The band rises to its magnitude at once and falls back at the rate `decay` asks for, so a hit
/// leaves something standing that then subsides. Unlike `band`, which is the raw magnitude the
/// analysis produced and has no ceiling, this is a share of the loudest thing heard lately: it
/// does not depend on how anyone set their input gain, and it does fall as the band falls.
fn band_held(index: i32) -> f32 {
    if index < 0 || index >= BANDS { return 0.0; }
    return textureLoad(analysis, vec2<i32>(BANDS + index, 1), 0).x;
}

/// The held level at a normalized position, interpolated the way `band_at` interpolates a band.
fn band_held_at(position: f32) -> f32 {
    let scaled = clamp(position, 0.0, 1.0) * f32(BANDS - 1);
    let low = i32(floor(scaled));
    return mix(band_held(low), band_held(low + 1), fract(scaled));
}

/// How far bass, mid and treble have each carried an animation, in turns.
///
/// Time that only passes while there is sound to make it pass. Each tone advances at a rate set by
/// how loud it is against the other two, scaled by `speed`, so a phase driven from this moves
/// because of the music rather than merely alongside it -- and in a silent room it stops. Use it
/// in place of `seconds` wherever an animation should be the music's doing and not the clock's.
fn tone_turns() -> vec3<f32> {
    return vec3<f32>(
        textureLoad(analysis, vec2<i32>(BANDS * 2, 1), 0).x,
        textureLoad(analysis, vec2<i32>(BANDS * 2 + 1, 1), 0).x,
        textureLoad(analysis, vec2<i32>(BANDS * 2 + 2, 1), 0).x,
    );
}

/// One waveform sample, `-1..1`.
fn wave(index: i32) -> f32 {
    if index < 0 || index >= WAVEFORM_POINTS { return 0.0; }
    return textureLoad(analysis, vec2<i32>(index, 0), 0).x;
}

fn wave_at(position: f32) -> f32 {
    let scaled = clamp(position, 0.0, 1.0) * f32(WAVEFORM_POINTS - 1);
    let low = i32(floor(scaled));
    return mix(wave(low), wave(low + 1), fract(scaled));
}

fn hash11(value: f32) -> f32 {
    return fract(sin(value * 127.1) * 43758.5453);
}

fn hash21(point: vec2<f32>) -> f32 {
    return fract(sin(dot(point, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn hash22(point: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(hash21(point), hash21(point + vec2<f32>(19.19, 7.77)));
}

fn value_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let smoothed = local * local * (3.0 - 2.0 * local);
    let a = hash21(cell);
    let b = hash21(cell + vec2<f32>(1.0, 0.0));
    let c = hash21(cell + vec2<f32>(0.0, 1.0));
    let d = hash21(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, smoothed.x), mix(c, d, smoothed.x), smoothed.y);
}

fn hue_to_rgb(hue: f32) -> vec3<f32> {
    let wrapped = fract(hue);
    let rgb = abs(vec3<f32>(wrapped * 6.0 - 3.0, wrapped * 6.0 - 2.0, wrapped * 6.0 - 4.0));
    return clamp(vec3<f32>(rgb.x - 1.0, 2.0 - rgb.y, 2.0 - rgb.z), vec3<f32>(0.0), vec3<f32>(1.0));
}

/// A soft edge one pixel wide, so nothing in a visualizer aliases into a flicker on a wall.
fn edge(distance: f32, width: f32) -> f32 {
    return 1.0 - smoothstep(width * 0.5, width * 0.5 + 0.004, abs(distance));
}

fn solid(distance: f32) -> f32 {
    return 1.0 - smoothstep(0.0, 0.004, distance);
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    // 0..1 across the output, y down, matching how a still image is sampled.
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Fragment {
    // Two triangles covering the output, built from the vertex index alone.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    var out: Fragment;
    out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    out.uv = corner;
    return out;
}

@fragment
fn fragment(input: Fragment) -> @location(0) vec4<f32> {
    // Centred coordinates, aspect-corrected, so a circle is round on any output.
    let centred = vec2<f32>((input.uv.x - 0.5) * aspect(), input.uv.y - 0.5) * 2.0;
    let colour = shade(centred, input.uv);
    return vec4<f32>(clamp(colour.rgb, vec3<f32>(0.0), vec3<f32>(4.0)), clamp(colour.a, 0.0, 1.0));
}
