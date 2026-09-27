// Project the original fifty-by-fifty terrain mesh at a sixty-degree tilt.
// Height changes the mesh, and wireframe follows its rows and columns.
fn terrain_vertex(x: f32, y: f32) -> vec3<f32> {
    let index = i32(y) * 50 + i32(x);
    let noise = textureLoad(analysis, vec2<i32>(index % 512, 2 + index / 512), 0).x;
    let depth = (y - 25.0) / 49.0 * 4.0 * zoom();
    let height = noise * size() * 2.0;
    // ofRotateXDeg(-60) on the original y-down canvas; the second component
    // is camera depth so overlapping triangles retain the frontmost surface.
    return vec3<f32>(depth * 0.5 + height * 0.8660254 + 400.0/1080.0,
        -depth * 0.8660254 + height * 0.5, noise);
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let column = p.x / (2.0 * aspect() * zoom()) * 49.0 + 25.0;
    if column < 0.0 || column > 49.0 { return vec4<f32>(0.0); }
    let x = min(floor(column), 48.0);
    let local_x = column - x;
    var nearest = -1.0e6;
    var a = terrain_vertex(x, 0.0);
    var b = terrain_vertex(x + 1.0, 0.0);
    var colour = vec4<f32>(0.0);
    let pixel = 2.0 / visualizer.resolution.y;
    for (var row = 0; row < 49; row++) {
        let y = f32(row);
        let c = terrain_vertex(x, y + 1.0);
        let d = terrain_vertex(x + 1.0, y + 1.0);
        for (var triangle = 0; triangle < 2; triangle++) {
            var start = mix(a, b, local_x);
            var end = mix(c, d, local_x);
            var lower = 0.0;
            var upper = 1.0;
            if triangle == 0 {
                end = mix(c, b, local_x);
                upper = 1.0 - local_x;
            } else {
                start = mix(c, b, local_x);
                lower = 1.0 - local_x;
            }
            let span = end.x - start.x;
            if abs(span) < 0.00001 { continue; }
            let t = (p.y - start.x) / span;
            if t < 0.0 || t > 1.0 { continue; }
            let point = mix(start, end, t);
            if point.y < nearest { continue; }
            nearest = point.y;
            let local_y = mix(lower, upper, t);
            let grid_x = min(local_x, 1.0 - local_x) * (2.0 * aspect() * zoom() / 49.0);
            let grid_y = min(local_y, 1.0 - local_y) * abs(mix(c.x - a.x, d.x - b.x, local_x));
            var alpha = 1.0;
            if wireframe() {
                alpha = 1.0 - smoothstep(pixel * 0.5, pixel * 1.5, min(grid_x, grid_y));
            }
            let brightness = max(max(primary().r, primary().g), primary().b);
            let tint = primary() / max(brightness, 0.00001);
            colour = vec4<f32>(tint * mix(50.0/255.0, 1.0, point.z), alpha);
        }
        a = c;
        b = d;
    }
    return colour;
}
