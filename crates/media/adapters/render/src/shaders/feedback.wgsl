struct Feedback {
    amount: f32,
    motion: f32,
    direction: f32,
    reset: f32,
    // A uniform struct is padded to 16 bytes on its own. Naming the tail as a vec3 instead would
    // align it to 16 first and push the struct to 48, which no longer matches the value written.
    delta_seconds: f32,
    clock_seconds: f32,
};

@group(0) @binding(0) var<uniform> feedback: Feedback;
@group(0) @binding(1) var live_source: texture_2d<f32>;
@group(0) @binding(2) var retained_source: texture_2d<f32>;
@group(0) @binding(3) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

const CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
);

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(CORNERS[index], 0.0, 1.0);
    output.uv = CORNERS[index] * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    return output;
}

fn retained_uv(uv: vec2<f32>) -> vec2<f32> {
    let step = feedback.motion * feedback.delta_seconds;
    let mode = u32(round(feedback.direction));
    if mode == 0u { return uv + vec2<f32>(0.0, step * 0.35); }
    if mode == 1u { return uv - vec2<f32>(0.0, step * 0.35); }
    if mode == 2u { return uv + vec2<f32>(step * 0.35, 0.0); }
    if mode == 3u { return uv - vec2<f32>(step * 0.35, 0.0); }
    if mode == 6u {
        let shake = vec2<f32>(
            sin(feedback.clock_seconds * 29.0),
            sin(feedback.clock_seconds * 37.0 + 1.7),
        );
        return uv + shake * step * 0.22;
    }
    if mode == 7u {
        let centred = uv - vec2<f32>(0.5);
        return centred * (1.0 - step * 0.32) + vec2<f32>(0.5);
    }
    let angle = select(-1.0, 1.0, mode == 4u) * step * 1.2;
    let sine = sin(angle);
    let cosine = cos(angle);
    let centred = uv - vec2<f32>(0.5);
    return vec2<f32>(
        centred.x * cosine - centred.y * sine,
        centred.x * sine + centred.y * cosine,
    ) + vec2<f32>(0.5);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let live = textureSample(live_source, source_sampler, in.uv);
    if feedback.reset > 0.5 || feedback.amount <= 0.0 {
        return live;
    }
    let previous_uv = retained_uv(in.uv);
    var retained = vec4<f32>(0.0);
    if previous_uv.x >= 0.0 && previous_uv.x <= 1.0 && previous_uv.y >= 0.0 && previous_uv.y <= 1.0 {
        retained = textureSample(retained_source, source_sampler, previous_uv);
    }
    // Keep a minimum live contribution even at the maximum setting so feedback remains a trail,
    // never a frozen frame. Repeated passes form the configurable temporal persistence.
    // Normalize persistence to a 60 Hz reference so the same amount has the same trail time on
    // every output cadence. The high ceiling permits a genuinely long trail without freezing.
    let frame_retention = clamp(feedback.amount, 0.0, 1.0) * 0.995;
    let retention = pow(frame_retention, max(feedback.delta_seconds * 60.0, 0.25));
    return mix(live, retained, retention);
}
