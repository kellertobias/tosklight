// Original Pixel implementation; no upstream code, textures or lookup tables are used.
// Trace null-ray orbits in their central plane using u'' = 1.5*u*u - u, with horizon radius 1.
// Finite orbit steps and a procedural thin disc approximate Schwarzschild lensing for live visuals.
fn black_hole_stars(p: vec2<f32>, scale: f32) -> vec3<f32> {
    let cell = floor(p * scale);
    let offset = hash22(cell + vec2<f32>(scale));
    let local = fract(p * scale) - mix(vec2<f32>(0.12), vec2<f32>(0.88), offset);
    let present = step(0.70, hash21(cell + vec2<f32>(41.0, scale)));
    let point = exp(-dot(local, local) * 560.0) * present;
    return mix(vec3<f32>(0.52, 0.68, 1.0), vec3<f32>(1.0, 0.84, 0.60), offset.x)
        * point * (0.35 + offset.y * 1.8);
}

fn black_hole_sky(direction: vec3<f32>) -> vec3<f32> {
    let sky = vec2<f32>(atan2(direction.z, direction.x) / TAU, asin(clamp(direction.y, -1.0, 1.0)) / TAU);
    let dust = value_noise(sky * 23.0) * 0.6 + value_noise(sky * 57.0) * 0.3;
    let band = exp(-pow((direction.y + sin(direction.x * 3.0) * 0.16) / 0.23, 2.0));
    return black_hole_stars(sky, 47.0) + black_hole_stars(sky, 113.0) * 0.6
        + vec3<f32>(0.025, 0.030, 0.05) * band * dust;
}

// Differentially rotating turbulence gives the disc fine, sheared filaments instead of rings.
fn black_hole_disc(position: vec3<f32>, direction: vec3<f32>, pulse: f32) -> vec4<f32> {
    let radial = length(position.xz);
    let inner = 3.0;
    let outer = 12.0;
    if radial <= inner || radial >= outer { return vec4<f32>(0.0); }
    let angle = atan2(position.z, position.x);
    let orbit = clock() * 1.7 * pow(inner / radial, 1.5);
    let advected = vec2<f32>(cos(angle - orbit), sin(angle - orbit)) * radial;
    let coarse = value_noise(advected * 1.3);
    let middle = value_noise(advected * 4.6 + vec2<f32>(coarse * 2.0));
    let fine = value_noise(advected * 15.0 + vec2<f32>(middle * 2.5));
    let filament = pow(0.5 + 0.5 * sin(radial * 20.0 + coarse * 7.0 + middle * 5.0), 2.0);
    let turbulence = 0.20 + coarse * 0.50 + middle * 0.25 + fine * 0.10 + filament * 0.15;
    let edge = smoothstep(inner, inner + 0.45, radial) * (1.0 - smoothstep(8.0, outer, radial));
    let temperature = pow(inner / radial, 0.75);
    let velocity = normalize(vec3<f32>(-position.z, 0.0, position.x)) * sqrt(0.5 / (radial - 1.0));
    let shift = sqrt(1.0 - 1.0 / radial) * sqrt(1.0 - dot(velocity, velocity))
        / max(1.0 - dot(velocity, -direction), 0.25);
    let hot = clamp(temperature * shift, 0.0, 1.0);
    let tint = mix(primary() * vec3<f32>(1.0, 0.65, 0.38), secondary(), pow(hot, 1.6));
    let light = edge * turbulence * pow(temperature, 2.2) * pow(shift, 3.0) * (3.4 + pulse * 4.0);
    return vec4<f32>(tint * light, edge * 0.88);
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let remaining = clamp(1.0 - beat_age(0) / max(decay(), 0.05), 0.0, 1.0);
    let pulse = remaining * remaining * max(reactivity(), 0.0);
    let shadow = clamp(size(), 0.08, 0.32) * (1.0 + pulse * 0.13);
    let observer_distance = 24.0;
    let elevation = mix(0.96, 0.035, sqrt(clamp(curvature(), 0.0, 1.0)));
    let radial_axis = vec3<f32>(0.0, elevation, -sqrt(1.0 - elevation * elevation));
    let up = normalize(cross(vec3<f32>(1.0, 0.0, 0.0), radial_axis));
    let camera = radial_axis * observer_distance;
    let ray = normalize(-radial_axis + (vec3<f32>(p.x, 0.0, 0.0) - up * p.y)
        * (2.598076 / (observer_distance * shadow)));
    let perpendicular = ray - radial_axis * dot(ray, radial_axis);
    let sine = length(perpendicular);
    let tangent_axis = perpendicular / max(sine, 0.000001);
    let impact = max(observer_distance * sine / sqrt(1.0 - 1.0 / observer_distance), 0.001);
    var inverse_radius = 1.0 / observer_distance;
    var slope = sqrt(max(1.0 / (impact * impact) - inverse_radius * inverse_radius
        + inverse_radius * inverse_radius * inverse_radius, 0.0));
    var radial = radial_axis;
    var tangent = tangent_axis;
    var previous = camera;
    var outgoing = ray;
    var radiance = vec3<f32>(0.0);
    var transmission = 1.0;
    var captured = false;
    // Second-order orbit integration. Noise runs only at disc intersections.
    let step_size = 0.045;
    let cosine = cos(step_size);
    let sine_step = sin(step_size);
    for (var step = 0; step < 144; step += 1) {
        let acceleration = 1.5 * inverse_radius * inverse_radius - inverse_radius;
        let next_inverse = inverse_radius + slope * step_size + acceleration * step_size * step_size * 0.5;
        let next_acceleration = 1.5 * next_inverse * next_inverse - next_inverse;
        slope += (acceleration + next_acceleration) * step_size * 0.5;
        inverse_radius = next_inverse;
        let next_radial = radial * cosine + tangent * sine_step;
        tangent = tangent * cosine - radial * sine_step;
        radial = next_radial;
        if inverse_radius <= 0.0001 { break; }
        let position = radial / inverse_radius;
        outgoing = normalize(position - previous);
        if previous.y * position.y <= 0.0 {
            let fraction = previous.y / (previous.y - position.y + 0.0000001);
            let crossing = mix(previous, position, clamp(fraction, 0.0, 1.0));
            let disc = black_hole_disc(crossing, outgoing, pulse);
            radiance += transmission * disc.rgb;
            transmission *= 1.0 - disc.a;
        }
        if inverse_radius >= 1.0 { captured = true; break; }
        if inverse_radius < 1.0 / observer_distance && slope < 0.0 { break; }
        previous = position;
    }
    if !captured { radiance += black_hole_sky(outgoing) * transmission; }
    // Higher-order light bunches at the critical impact parameter. Restrained bloom keeps
    // the traced secondary disc legible instead of covering it with a solid white outline.
    let ring_distance = abs(impact - 2.598076);
    let ring = exp(-ring_distance / 0.045) * 0.65 + exp(-ring_distance / 0.20) * 0.06;
    radiance += secondary() * ring * (1.0 + pulse * 1.6) * smoothstep(2.4, 2.6, impact);
    return vec4<f32>(vec3<f32>(1.0) - exp(-radiance * 1.25), 1.0);
}
