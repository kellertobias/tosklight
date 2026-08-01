struct Uniforms {
    viewport_time: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct SceneHit {
    distance: f32,
    material: f32,
};

struct RayHit {
    distance: f32,
    material: f32,
};

struct MovingLight {
    position: vec3<f32>,
    direction: vec3<f32>,
    color: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = output.position.xy * 0.5 + 0.5;
    return output;
}

fn rotate_y(point: vec3<f32>, angle: f32) -> vec3<f32> {
    let cosine = cos(angle);
    let sine = sin(angle);
    return vec3<f32>(
        cosine * point.x - sine * point.z,
        point.y,
        sine * point.x + cosine * point.z,
    );
}

fn box_sdf(point: vec3<f32>, half_size: vec3<f32>) -> f32 {
    let delta = abs(point) - half_size;
    return length(max(delta, vec3<f32>(0.0))) + min(max(delta.x, max(delta.y, delta.z)), 0.0);
}

fn capped_cylinder_sdf(point: vec3<f32>, radius: f32, half_height: f32) -> f32 {
    let delta = abs(vec2<f32>(length(point.xz), point.y)) - vec2<f32>(radius, half_height);
    return min(max(delta.x, delta.y), 0.0) + length(max(delta, vec2<f32>(0.0)));
}

fn union_hit(current: SceneHit, distance: f32, material: f32) -> SceneHit {
    if distance < current.distance {
        return SceneHit(distance, material);
    }
    return current;
}

fn fixture_sdf(point: vec3<f32>, side: f32, time: f32) -> SceneHit {
    let center = vec3<f32>(side * 4.25, 0.0, 0.55);
    var hit = SceneHit(1000.0, 0.0);
    hit = union_hit(hit, box_sdf(point - center - vec3<f32>(0.0, 0.22, 0.0), vec3<f32>(0.72, 0.22, 0.68)), 4.0);
    hit = union_hit(hit, capped_cylinder_sdf(point - center - vec3<f32>(0.0, 0.68, 0.0), 0.42, 0.22), 4.0);

    let head_center = center + vec3<f32>(0.0, 2.62, 0.0);
    let pan = side * (0.34 + sin(time * 0.52 + side) * 0.42);
    let local = rotate_y(point - head_center, pan);
    hit = union_hit(hit, capped_cylinder_sdf(local, 0.5, 0.48), 5.0);
    hit = union_hit(hit, length(local - vec3<f32>(0.0, -0.03, -0.45)) - 0.27, 6.0 + step(0.0, side));

    let post_left = point - center - vec3<f32>(-0.48, 1.55, 0.0);
    let post_right = point - center - vec3<f32>(0.48, 1.55, 0.0);
    hit = union_hit(hit, box_sdf(post_left, vec3<f32>(0.10, 0.92, 0.18)), 4.0);
    hit = union_hit(hit, box_sdf(post_right, vec3<f32>(0.10, 0.92, 0.18)), 4.0);
    return hit;
}

fn scene_sdf(point: vec3<f32>, time: f32) -> SceneHit {
    var hit = SceneHit(point.y, 1.0);
    let cube_point = rotate_y(point - vec3<f32>(0.0, 1.18, 0.0), time * 0.16);
    hit = union_hit(hit, box_sdf(cube_point, vec3<f32>(1.15)), 2.0);
    hit = union_hit(hit, box_sdf(point - vec3<f32>(0.0, 0.16, 0.0), vec3<f32>(5.8, 0.16, 3.8)), 3.0);
    let left = fixture_sdf(point, -1.0, time);
    let right = fixture_sdf(point, 1.0, time);
    hit = union_hit(hit, left.distance, left.material);
    hit = union_hit(hit, right.distance, right.material);
    return hit;
}

fn light_color(phase: f32) -> vec3<f32> {
    return 0.48 + 0.52 * cos(vec3<f32>(0.0, 2.1, 4.2) + phase);
}

fn moving_light(index: i32, time: f32) -> MovingLight {
    let side = select(-1.0, 1.0, index == 1);
    let position = vec3<f32>(side * 4.25, 3.1, 0.20);
    let aim_point = vec3<f32>(
        sin(time * 0.58 + side * 1.1) * 2.15,
        0.65 + sin(time * 0.39 + side * 2.4) * 0.48,
        sin(time * 0.31 + side * 0.7) * 1.55,
    );
    let direction = normalize(aim_point - position);
    let phase = time * 0.42 + select(0.0, 2.7, index == 1);
    return MovingLight(position, direction, light_color(phase));
}

fn raymarch(origin: vec3<f32>, direction: vec3<f32>, time: f32) -> RayHit {
    var distance = 0.0;
    var material = 0.0;
    for (var step = 0; step < 112; step += 1) {
        let sample = scene_sdf(origin + direction * distance, time);
        if sample.distance < 0.0015 || distance > 28.0 {
            material = sample.material;
            break;
        }
        distance += sample.distance * 0.72;
    }
    return RayHit(distance, material);
}

fn normal_at(point: vec3<f32>, time: f32) -> vec3<f32> {
    let epsilon = 0.002;
    let center = scene_sdf(point, time).distance;
    return normalize(vec3<f32>(
        scene_sdf(point + vec3<f32>(epsilon, 0.0, 0.0), time).distance - center,
        scene_sdf(point + vec3<f32>(0.0, epsilon, 0.0), time).distance - center,
        scene_sdf(point + vec3<f32>(0.0, 0.0, epsilon), time).distance - center,
    ));
}

fn soft_shadow(origin: vec3<f32>, direction: vec3<f32>, maximum: f32, time: f32) -> f32 {
    var travel = 0.035;
    var visibility = 1.0;
    for (var step = 0; step < 36; step += 1) {
        let distance = scene_sdf(origin + direction * travel, time).distance;
        visibility = min(visibility, 14.0 * distance / travel);
        travel += clamp(distance, 0.018, 0.32);
        if distance < 0.001 || travel > maximum {
            break;
        }
    }
    return clamp(visibility, 0.0, 1.0);
}

fn box_segment_clear(origin: vec3<f32>, destination: vec3<f32>, time: f32) -> f32 {
    let ray = destination - origin;
    let local_origin = rotate_y(origin - vec3<f32>(0.0, 1.18, 0.0), time * 0.16);
    let local_ray = rotate_y(ray, time * 0.16);
    let inverse = 1.0 / (local_ray + sign(local_ray) * vec3<f32>(0.00001));
    let first = (-vec3<f32>(1.18) - local_origin) * inverse;
    let second = (vec3<f32>(1.18) - local_origin) * inverse;
    let near_values = min(first, second);
    let far_values = max(first, second);
    let near_hit = max(near_values.x, max(near_values.y, near_values.z));
    let far_hit = min(far_values.x, min(far_values.y, far_values.z));
    if far_hit >= max(near_hit, 0.0) && near_hit <= 1.0 {
        return 0.08;
    }
    return 1.0;
}

fn spotlight(light: MovingLight, point: vec3<f32>) -> vec2<f32> {
    let from_light = point - light.position;
    let distance = length(from_light);
    let alignment = dot(normalize(from_light), light.direction);
    let cone = smoothstep(0.91, 0.965, alignment);
    let attenuation = 1.0 / (1.0 + distance * distance * 0.045);
    return vec2<f32>(cone * attenuation, distance);
}

fn material_color(material: f32, point: vec3<f32>, time: f32) -> vec3<f32> {
    if material < 1.5 {
        let checker = (i32(floor(point.x)) + i32(floor(point.z))) & 1;
        return select(vec3<f32>(0.075, 0.085, 0.10), vec3<f32>(0.12, 0.13, 0.15), checker == 1);
    }
    if material < 2.5 {
        return vec3<f32>(0.42, 0.45, 0.50);
    }
    if material < 3.5 {
        return vec3<f32>(0.055, 0.06, 0.07);
    }
    if material < 5.5 {
        return vec3<f32>(0.025, 0.03, 0.035);
    }
    if material < 6.5 {
        return moving_light(0, time).color * 2.2;
    }
    return moving_light(1, time).color * 2.2;
}

fn shade_surface(point: vec3<f32>, normal: vec3<f32>, view_direction: vec3<f32>, material: f32, time: f32) -> vec3<f32> {
    let base = material_color(material, point, time);
    if material > 5.5 {
        return base;
    }

    var color = base * vec3<f32>(0.055, 0.065, 0.085);
    for (var index = 0; index < 2; index += 1) {
        let light = moving_light(index, time);
        let to_light = light.position - point;
        let distance = length(to_light);
        let light_direction = to_light / distance;
        let spot = spotlight(light, point).x;
        let shadow = soft_shadow(point + normal * 0.018, light_direction, distance, time);
        let diffuse = max(dot(normal, light_direction), 0.0);
        let half_vector = normalize(light_direction - view_direction);
        let specular = pow(max(dot(normal, half_vector), 0.0), 42.0) * 0.34;
        color += (base * diffuse + specular) * light.color * spot * shadow * 5.2;
    }
    return color;
}

fn fog_scattering(origin: vec3<f32>, direction: vec3<f32>, maximum: f32, time: f32) -> vec3<f32> {
    let steps = 48;
    let step_size = maximum / f32(steps);
    var scattering = vec3<f32>(0.0);
    for (var index = 0; index < steps; index += 1) {
        let travel = (f32(index) + 0.45) * step_size;
        let point = origin + direction * travel;
        let height_fade = exp(-max(point.y, 0.0) * 0.055);
        let haze = 0.82 + 0.18 * sin(point.x * 2.1 + point.z * 1.7 + time * 0.28);
        for (var light_index = 0; light_index < 2; light_index += 1) {
            let light = moving_light(light_index, time);
            let spot = spotlight(light, point);
            let clear = box_segment_clear(point, light.position, time);
            let view_phase = 0.55 + 0.45 * pow(max(dot(direction, normalize(light.position - point)), 0.0), 2.0);
            scattering += light.color * spot.x * clear * view_phase * height_fade * haze * step_size * 0.135;
        }
    }
    return scattering;
}

fn filmic(color: vec3<f32>) -> vec3<f32> {
    let mapped = color / (color + vec3<f32>(1.0));
    return pow(mapped, vec3<f32>(1.0 / 2.2));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let resolution = uniforms.viewport_time.xy;
    let time = uniforms.viewport_time.z;
    var screen = input.uv * 2.0 - 1.0;
    screen.x *= resolution.x / resolution.y;

    let origin = vec3<f32>(0.0, 4.35, 11.8);
    let aim_point = vec3<f32>(0.0, 1.35, 0.0);
    let forward = normalize(aim_point - origin);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let direction = normalize(forward + right * screen.x * 0.72 + up * screen.y * 0.72);

    let hit = raymarch(origin, direction, time);
    let fog_limit = min(hit.distance, 25.0);
    let fog = fog_scattering(origin, direction, fog_limit, time);

    var surface = vec3<f32>(0.006, 0.009, 0.016);
    if hit.distance < 28.0 {
        let point = origin + direction * hit.distance;
        let normal = normal_at(point, time);
        surface = shade_surface(point, normal, direction, hit.material, time);
    }

    let atmosphere = exp(-fog_limit * 0.018);
    let vignette = 1.0 - 0.24 * dot(input.uv - 0.5, input.uv - 0.5);
    return vec4<f32>(filmic((surface * atmosphere + fog) * vignette), 1.0);
}
