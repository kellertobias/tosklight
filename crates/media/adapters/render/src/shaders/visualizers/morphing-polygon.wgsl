// A polygon whose vertices are pushed outward by the spectrum.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let sides = max(floor(count()), 3.0);
    let distance = length(p);
    let angle = atan2(p.y, p.x);
    let segment = angle / TAU + 0.5;

    // The polygon's edge distance, then a per-vertex deformation from the bands.
    let wedge = TAU / sides;
    let folded = abs(fract(angle / wedge + 0.5) - 0.5) * wedge;
    let deformation = band_at(fract(segment * sides) * 0.5 + segment * 0.5)
        * amount() * reactivity();
    let edge_radius = (radius() + deformation * 0.5) * cos(wedge * 0.5) / max(cos(folded), 0.001);

    var alpha = 0.0;
    if filled() {
        alpha = solid(distance - edge_radius);
    } else {
        alpha = edge(distance - edge_radius, thickness() * 6.0);
    }
    return vec4<f32>(mix(primary(), secondary(), deformation), alpha);
}
