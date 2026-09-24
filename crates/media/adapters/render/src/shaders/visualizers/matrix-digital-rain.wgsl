// Falling procedural code columns. Each cell contains a stable pseudo-glyph; column speeds and
// trail lengths vary independently so the result reads as digital rain rather than a regular grid.
// Every landed beat sends `burst()` bright streaks down columns of their own.

/// How far down the picture a beat's streak falls in one second, at speed one.
const STREAK_FALL: f32 = 0.9;
/// How long a beat's trail is, as a share of the picture's height.
const STREAK_TRAIL: f32 = 0.6;

/// How brightly the beat streaks light this column at this height: the trail, then the head.
fn beat_streaks(column: f32, columns: f32, y: f32) -> vec2<f32> {
    var trail = 0.0;
    var head = 0.0;
    let streaks = min(burst(), 8.0);
    if streaks < 0.5 { return vec2<f32>(0.0); }
    let fall = STREAK_FALL * max(speed(), 0.05);
    var index = 0;
    loop {
        if index >= BEAT_HISTORY { break; }
        let age = beat_age(index);
        // Past the bottom with its whole trail: nothing older is still falling either.
        if age * fall > 1.0 + STREAK_TRAIL { break; }
        let seed = beat_count() - f32(index);
        var streak = 0.0;
        loop {
            if streak >= streaks { break; }
            let chosen = floor(hash21(vec2<f32>(seed * 1.37, streak * 7.13 + 3.1)) * columns);
            if abs(chosen - column) < 0.5 {
                // Each streak of a burst starts a little apart, so several read as a volley.
                let delay = hash11(seed * 5.3 + streak * 2.9) * 0.12;
                let head_y = (age - delay) * fall;
                let behind = head_y - y;
                if behind >= 0.0 && behind < STREAK_TRAIL {
                    trail = max(trail, 1.0 - behind / STREAK_TRAIL);
                    head = max(head, 1.0 - smoothstep(0.0, 0.03, behind));
                }
            }
            streak += 1.0;
        }
        index += 1;
    }
    return vec2<f32>(trail, head);
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let columns = clamp(round(count()), 8.0, 160.0);
    let rows = columns / max(aspect(), 0.25) * 0.62;
    let cell = vec2<f32>(uv.x * columns, uv.y * rows);
    let cell_id = floor(cell);
    let local = fract(cell);
    let column_seed = hash11(cell_id.x + 17.0);
    // The flow already carries speed and the smoothed energy, and only moves forward: louder music
    // makes the rain fall faster rather than jumping it to a different place.
    let rate = mix(0.16, 0.52, column_seed);
    let travelled = flow() * rate;
    let head = fract(travelled + hash11(cell_id.x * 3.71));
    let trail_distance = fract(head - uv.y);
    let trail_length = mix(0.18, 0.62, hash11(cell_id.x * 8.13));
    let trail = 1.0 - smoothstep(0.0, trail_length, trail_distance);
    let head_glow = 1.0 - smoothstep(0.0, 0.035, abs(trail_distance));

    // Refresh a column's character sequence as the head advances. The strokes form small blocky
    // code glyphs, including vertical stems and independently selected cross-bars.
    let tick = floor(travelled * rows);
    let glyph_seed = hash21(vec2<f32>(cell_id.x * 11.7, cell_id.y + tick));
    let stem = 1.0 - smoothstep(0.10, 0.18, abs(local.x - mix(0.32, 0.68, glyph_seed)));
    let bar_a = (1.0 - smoothstep(0.08, 0.16, abs(local.y - 0.26)))
        * step(0.28, hash11(glyph_seed * 31.0));
    let bar_b = (1.0 - smoothstep(0.08, 0.16, abs(local.y - 0.72)))
        * step(0.45, hash11(glyph_seed * 67.0));
    let side = (1.0 - smoothstep(0.08, 0.16, abs(local.x - 0.76)))
        * step(0.62, hash11(glyph_seed * 97.0));
    let glyph = clamp(max(stem, max(bar_a, max(bar_b, side))), 0.0, 1.0)
        * smoothstep(0.04, 0.12, local.y)
        * smoothstep(0.04, 0.12, 1.0 - local.y);

    // The level lifts the rain gently; the beat is carried by the streaks, never by a flicker.
    let audio_light = 0.65 + smooth_energy() * 0.55;
    let body = mix(secondary(), primary(), trail) * trail;
    let rain = body + primary() * head_glow * 1.25;
    let brightness = mix(0.18, 1.5, clamp(amount(), 0.0, 1.0)) * audio_light;

    let streak = beat_streaks(cell_id.x, columns, uv.y);
    // A beat's streak outshines the rain: brighter, paler, and lit along its whole trail.
    let sent = mix(primary(), vec3<f32>(1.0), 0.25) * streak.x * 2.4
        + mix(primary(), vec3<f32>(1.0), 0.8) * streak.y * 3.0;
    let colour = rain * brightness + sent * mix(0.5, 1.2, clamp(amount(), 0.0, 1.0));
    return vec4<f32>(colour * glyph, 1.0);
}
