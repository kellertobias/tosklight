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
    // burst, spare, spare, spare
    params4: vec4<f32>,
    // mirror, filled, wireframe, on beat
    flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> visualizer: Visualizer;
// Row 0 is the 512-point waveform. Row 1 holds the 64 spectrum bands in its first texels, then what
// is held of the kick, snare and hi-hat hits, then how far each of those has carried an animation,
// then the rhythm: smoothed levels, beat pulse, eased and whole beat counts, the flow and speed
// clocks, and the age of each recent beat.
@group(0) @binding(1) var analysis: texture_2d<f32>;

const WAVEFORM_POINTS: i32 = 512;
const BANDS: i32 = 64;
const TAU: f32 = 6.2831853;
const RHYTHM: i32 = BANDS + 6;
const BEAT_HISTORY: i32 = 16;

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
/// The operator chose to have this visualizer move with the landed beat, not the audio's level.
fn on_beat() -> bool { return visualizer.flags.w > 0.5; }
/// How many things one beat sends.
fn burst() -> f32 { return visualizer.params4.x; }

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

/// What is left of the last kick, snare and hi-hat, each `0..1`.
///
/// The detector's own hit flash, `kick()` and its siblings, is gone within about a tenth of a
/// second, which is right for a lamp and too quick to move anything. This rises with the strike and
/// then falls back at the rate `decay` asks for, so a hit can push something that visibly eases out.
fn held_hits() -> vec3<f32> {
    return vec3<f32>(
        textureLoad(analysis, vec2<i32>(BANDS, 1), 0).x,
        textureLoad(analysis, vec2<i32>(BANDS + 1, 1), 0).x,
        textureLoad(analysis, vec2<i32>(BANDS + 2, 1), 0).x,
    );
}

/// How far the kick, snare and hi-hat have each carried an animation, in turns.
///
/// Time that only passes on the beat. Each advances while its hit is held, at a rate graded from
/// the kick down to the hi-hat and scaled by `speed`, so a phase driven from this moves because the
/// instrument struck -- and between songs, or on a dead input, it stops. Use it in place of
/// `seconds` wherever an animation should be the music's doing and not the clock's.
fn hit_turns() -> vec3<f32> {
    return vec3<f32>(
        textureLoad(analysis, vec2<i32>(BANDS + 3, 1), 0).x,
        textureLoad(analysis, vec2<i32>(BANDS + 4, 1), 0).x,
        textureLoad(analysis, vec2<i32>(BANDS + 5, 1), 0).x,
    );
}

fn rhythm(index: i32) -> f32 {
    return textureLoad(analysis, vec2<i32>(RHYTHM + index, 1), 0).x;
}

/// Bass, mid, treble, energy and peak, eased at the rate `smoothing` asks for. Zero smoothing is
/// the level as it is.
fn smooth_bass() -> f32 { return rhythm(0); }
fn smooth_mid() -> f32 { return rhythm(1); }
fn smooth_treble() -> f32 { return rhythm(2); }
fn smooth_energy() -> f32 { return rhythm(3); }
fn smooth_peak() -> f32 { return rhythm(4); }

/// `1.0` when a beat lands, easing back to zero; `smoothing` sets how slowly.
fn beat_pulse() -> f32 { return rhythm(5); }

/// The number of landed beats, eased: it climbs by one on every beat, smoothly. A phase read from
/// it steps forward on the beat and stands still between songs.
fn beat_steps() -> f32 { return rhythm(6); }

/// The number of landed beats. Beat `beat_count() - i` is the one `beat_age(i)` describes, which
/// makes it a stable seed for whatever that beat sent.
fn beat_count() -> f32 { return rhythm(7); }

/// Seconds at `speed`, sped up by the smoothed energy. It only goes forward, so a phase read from
/// it glides faster when the music is louder instead of jumping the way `seconds() * energy()`
/// would.
fn flow() -> f32 { return rhythm(8); }

/// Seconds at `speed`. Moving the speed fader changes the rate and never the position.
fn clock() -> f32 { return rhythm(9); }

/// Seconds since the `index`-th most recent beat, newest first. Very large for a beat that never
/// landed, so anything it would have sent is long gone.
fn beat_age(index: i32) -> f32 {
    if index < 0 || index >= BEAT_HISTORY { return 1.0e6; }
    return rhythm(10 + index);
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
