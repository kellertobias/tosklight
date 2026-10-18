// Original live visualizer inspired by mrsarac/ff-tracking's filmed terminal and glyph locks.
// Glyph boxes, reading-order links, projected coordinate labels and red beat locks follow the
// actual typed characters. A ray intersects a tilted screen; focus follows the newest glyph.
fn tracking_glyph(code: i32, uv: vec2<f32>) -> f32 {
    if any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0)) { return 0.0; }
    let glyph = clamp(code, 32, 127) - 32;
    let cell = vec2<i32>(glyph % 16, glyph / 16);
    return textureLoad(tracking_glyphs, cell * vec2<i32>(16, 32) + vec2<i32>(uv * vec2<f32>(16.0, 32.0)), 0).a;
}
fn tracking_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, width: f32) -> f32 {
    let delta = b - a;
    let t = clamp(dot(p - a, delta) / max(dot(delta, delta), 0.00001), 0.0, 1.0);
    return 1.0 - smoothstep(width, width * 2.0, length(p - a - delta * t));
}
fn tracking_project(p: vec2<f32>, pan: vec2<f32>, tilt: vec2<f32>, zoomed: f32) -> vec2<f32> {
    let screen = (p - pan) * (2.0 * zoomed) / max(2.0 - dot(tilt, p), 0.5);
    return (screen / vec2<f32>(aspect(), 1.0) + 0.5) * visualizer.resolution.xy;
}
fn shade(_centred: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let total = i32(textureLoad(analysis, vec2<i32>(0, 6), 0).x);
    if total == 0 { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    let columns = i32(clamp(count(), 4.0, 32.0));
    let rows = ceil(f32(total) / f32(columns));
    let pulse = max(beat(), beat_pulse());
    // Start with a readable partial line, then type and lock from the musical clock.
    let progress = fract((clock() * 0.45 + beat_steps() * 0.12) / max(f32(total) * 0.12, 1.0));
    let visible = min(total, max(1, i32((0.32 + progress * 0.95) * f32(total))));
    var last = visible - 1;
    for (var skip = 0; skip < 32; skip += 1) {
        if last <= 0 || textureLoad(analysis, vec2<i32>(last + 1, 6), 0).x != 32.0 { break; }
        last -= 1;
    }
    let spread = 1.0 + amount() * 0.6 + smooth_bass() * reactivity() * 0.16;
    let height = min(clamp(size(), 0.035, 0.3), 0.78 / max(rows * 1.45, 1.0));
    let advance = min(height * 0.64 * spread, aspect() * 0.76 / f32(columns));
    let line = height * 1.55;
    let origin = vec2<f32>(-f32(columns) * advance * 0.5, -rows * line * 0.5);
    let focus_point = origin + vec2<f32>((f32(last % columns) + 0.5) * advance, (f32(last / columns) + 0.5) * line);
    let shot = floor(beat_count() / 2.0);
    let motion = curvature();
    let tilt = vec2<f32>(sin(shot * 2.1 + clock() * 0.13), cos(shot * 1.3)) * motion * 0.7;
    let pan = focus_point * motion * 0.24;
    let zoomed = 1.0 + pulse * motion * 0.10;
    var screen = (uv - 0.5) * vec2<f32>(aspect(), 1.0);
    // The screen tears only on a lock, and the real projected labels use that same shift.
    screen.x += sin(floor(uv.y * 90.0) * 2.7) * pulse * motion * 0.003;
    let ray = (2.0 - dot(tilt, pan)) / max(2.0 * zoomed + dot(tilt, screen), 0.4);
    let point = pan + screen * ray;
    let cell = vec2<i32>(floor((point - origin) / vec2<f32>(advance, line)));
    let index = cell.y * columns + cell.x;
    let within = cell.x >= 0 && cell.x < columns && cell.y >= 0 && index >= 0 && index < visible;
    let centre = origin + (vec2<f32>(cell) + 0.5) * vec2<f32>(advance, line);
    let local = (point - centre) / vec2<f32>(advance, height) + 0.5;
    var ink = 0.0;
    if within {
        let code = i32(textureLoad(analysis, vec2<i32>(index + 1, 6), 0).x);
        // Five taps approximate a rack focus on the tracked glyph; boxes remain sharp.
        let blur = min(abs(f32(index - last)) * motion * 0.008, 0.045);
        ink = tracking_glyph(code, local) * 0.5;
        ink += tracking_glyph(code, local + vec2<f32>(blur, 0.0)) * 0.125;
        ink += tracking_glyph(code, local - vec2<f32>(blur, 0.0)) * 0.125;
        ink += tracking_glyph(code, local + vec2<f32>(0.0, blur)) * 0.125;
        ink += tracking_glyph(code, local - vec2<f32>(0.0, blur)) * 0.125;
    }
    let pixel = 1.25 / max(visualizer.resolution.y, 1.0);
    var boxes = 0.0;
    var lock_ink = 0.0;
    var labels = 0.0;
    for (var history = 0; history < 5; history += 1) {
        let tracked = last - history;
        if tracked < 0 { break; }
        let at = origin + vec2<f32>((f32(tracked % columns) + 0.5) * advance, (f32(tracked / columns) + 0.5) * line);
        let extent = vec2<f32>(advance * 0.52, height * 0.57);
        let distance = abs(point - at) - extent;
        let border = 1.0 - smoothstep(pixel, pixel * 2.5, abs(max(distance.x, distance.y)));
        boxes = max(boxes, border * (1.0 - f32(history) * 0.13));
        if history == 0 { lock_ink = border * step(0.35, pulse); }
        if history > 0 {
            let previous = tracked + 1;
            let next = origin + vec2<f32>((f32(previous % columns) + 0.5) * advance, (f32(previous / columns) + 0.5) * line);
            let a = at + vec2<f32>(0.0, -height * 0.67);
            let b = next + vec2<f32>(0.0, -height * 0.67);
            boxes = max(boxes, tracking_segment(point, a, b, pixel) * 0.65);
            let dot_at = mix(a, b, fract(clock() * 0.8 + f32(history) * 0.2));
            boxes = max(boxes, 1.0 - smoothstep(pixel * 2.0, pixel * 4.0, length(point - dot_at)));
        }
        // Position labels are the actual projection in output pixels, x then y, four digits each.
        let output_position = tracking_project(at, pan, tilt, zoomed);
        let tiny = vec2<f32>(height * 0.13, height * 0.30);
        let label_uv = (point - at - vec2<f32>(-advance * 0.5, height * 0.62)) / tiny;
        let digit = i32(floor(label_uv.x));
        if history == 0 && label_uv.y >= 0.0 && label_uv.y < 1.0 && digit >= 0 && digit < 13 {
            var code = 32;
            if digit == 0 { code = 120; }
            if digit == 1 || digit == 8 { code = 58; }
            if digit == 7 { code = 121; }
            if (digit >= 2 && digit <= 5) || digit >= 9 {
                let coordinate = select(output_position.x, output_position.y, digit >= 9);
                let power = select(5 - digit, 12 - digit, digit >= 9);
                code = 48 + (i32(abs(coordinate) / pow(10.0, f32(power))) % 10);
            }
            labels = max(labels, tracking_glyph(code, vec2<f32>(fract(label_uv.x), label_uv.y)));
        }
    }
    let glow = primary() * ink + secondary() * (boxes * 0.7 + labels * 0.65);
    let paper = mode() > 0.5;
    var colour = select(vec3<f32>(0.004, 0.009, 0.008) + glow, vec3<f32>(0.95, 0.94, 0.9) - glow * 0.85, paper);
    colour = mix(colour, vec3<f32>(1.0, 0.025, 0.015), lock_ink);
    let scan = 0.97 + 0.03 * sin(uv.y * visualizer.resolution.y * 3.14159);
    return vec4<f32>(max(colour * scan, vec3<f32>(0.0)), 1.0);
}
