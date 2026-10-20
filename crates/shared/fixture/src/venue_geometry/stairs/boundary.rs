//! Cancel split edges between subtraction pieces to keep only the physical slab contour.
use std::collections::BTreeMap;
pub(super) fn boundary(pieces: &[Vec<[f64; 2]>]) -> Vec<[[f64; 2]; 2]> {
    let vertices: Vec<_> = pieces.iter().flatten().copied().collect();
    let mut segments = BTreeMap::new();
    for polygon in pieces {
        for i in 0..polygon.len() {
            let a = polygon[i];
            let b = polygon[(i + 1) % polygon.len()];
            let delta = [b[0] - a[0], b[1] - a[1]];
            let squared = delta[0] * delta[0] + delta[1] * delta[1];
            if squared < 1e-12 {
                continue;
            }
            let mut cuts = vec![0.0, 1.0];
            for p in &vertices {
                let t = ((p[0] - a[0]) * delta[0] + (p[1] - a[1]) * delta[1]) / squared;
                let distance =
                    ((p[0] - a[0]) * delta[1] - (p[1] - a[1]) * delta[0]).abs() / squared.sqrt();
                if t > 1e-9 && t < 1.0 - 1e-9 && distance < 1e-5 {
                    cuts.push(t);
                }
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            for interval in cuts.windows(2) {
                let points =
                    [interval[0], interval[1]].map(|t| [a[0] + delta[0] * t, a[1] + delta[1] * t]);
                let key = points.map(|p| {
                    (
                        (p[0] * 10000.0).round() as i64,
                        (p[1] * 10000.0).round() as i64,
                    )
                });
                let key = if key[0] < key[1] {
                    (key[0], key[1])
                } else {
                    (key[1], key[0])
                };
                let value = segments.entry(key).or_insert((0, points));
                value.0 += 1;
            }
        }
    }
    segments
        .into_values()
        .filter_map(|(count, line)| (count == 1).then_some(line))
        .collect()
}
