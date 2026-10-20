pub(super) fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
pub(super) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(super) fn mul(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|v| v * s)
}
pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub(super) fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
pub(super) fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    length(sub(a, b))
}
pub(super) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(super) fn normal(t: [[f64; 3]; 3]) -> [f64; 3] {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let l = length(n);
    if l > 1e-8 { mul(n, 1.0 / l) } else { [0.0; 3] }
}
pub(super) fn cross2(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[2] - a[2] * b[0]
}
pub(super) fn axis(m: [[f64; 4]; 4], i: usize) -> [f64; 3] {
    [m[i][0], m[i][1], m[i][2]]
}
pub(super) fn transform(m: [[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| m[0][i] * p[0] + m[1][i] * p[1] + m[2][i] * p[2] + m[3][i])
}
pub(super) fn inverse(m: [[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    let p = sub(p, axis(m, 3));
    std::array::from_fn(|i| dot(axis(m, i), p))
}
pub(super) fn plan_distance(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(b, a);
    let d = [d[0], 0.0, d[2]];
    let p = sub(p, a);
    let p = [p[0], 0.0, p[2]];
    let t = (dot(p, d) / dot(d, d)).clamp(0.0, 1.0);
    length(sub(p, mul(d, t)))
}
