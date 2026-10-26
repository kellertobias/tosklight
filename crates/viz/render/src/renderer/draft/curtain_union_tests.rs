//! Independent ray-clipping/coverage reference: all blockers survive the bounded admission path.
fn interval(planes: &[(f32, f32)], first: f32, last: f32) -> Option<(f32, f32)> {
    let mut a = first;
    let mut b = last;
    for &(origin, slope) in planes {
        if slope == 0.0 {
            if origin < 0.0 {
                return None;
            }
        } else if slope > 0.0 {
            a = a.max(-origin / slope);
        } else {
            b = b.min(-origin / slope);
        }
    }
    (b > a).then_some((a, b))
}
fn admitted(candidates: &[Option<(f32, f32)>]) -> Option<Vec<(f32, f32)>> {
    let mut union: Vec<(f32, f32)> = Vec::new();
    for &(a, b) in candidates.iter().flatten() {
        if b <= a {
            continue;
        }
        let mut merged = (a, b);
        let mut insert = 0;
        while insert < union.len() {
            if union[insert].1 < merged.0 {
                insert += 1;
                continue;
            }
            if union[insert].0 > merged.1 {
                break;
            }
            let previous = union.remove(insert);
            merged = (merged.0.min(previous.0), merged.1.max(previous.1));
        }
        if union.len() == 8 {
            return None;
        }
        union.insert(insert, merged);
    }
    Some(union)
}
// Independently partition every endpoint; midpoint coverage integrates the exact piecewise union.
fn reference_visible(candidates: &[Option<(f32, f32)>], first: f32, last: f32) -> f64 {
    let mut endpoints = vec![first, last];
    for &(a, b) in candidates.iter().flatten() {
        endpoints.extend([a.max(first).min(last), b.max(first).min(last)]);
    }
    endpoints.sort_by(f32::total_cmp);
    endpoints.dedup();
    endpoints
        .windows(2)
        .filter_map(|p| {
            // f64 midpoint preserves even a single-ULP f32 gap in the independent reference.
            let middle = (f64::from(p[0]) + f64::from(p[1])) * 0.5;
            (!candidates
                .iter()
                .flatten()
                .any(|&(a, b)| f64::from(a) <= middle && middle <= f64::from(b)))
            .then_some((p[0], p[1]))
        })
        .map(|(a, b)| primitive(b) - primitive(a))
        .sum()
}
fn primitive(t: f32) -> f64 {
    (f64::from(t) + 2.0).ln()
}
fn visible(union: &[(f32, f32)], first: f32, last: f32) -> f64 {
    let mut cursor = first;
    let mut value = 0.0;
    for &(a, b) in union {
        if a > cursor {
            value += primitive(a) - primitive(cursor);
        }
        cursor = cursor.max(b);
    }
    value + primitive(last) - primitive(cursor)
}
#[test]
fn many_candidates_with_empty_overlapping_and_touching_ray_intervals_retain_exact_coverage() {
    let mut candidates = vec![None; 20];
    // Independent halfspaces clip a ray into finite shadow intervals.
    for &(a, b) in &[
        (1.0, 3.0),
        (2.0, 4.0),
        (4.0, 5.0),
        (8.0, 9.0),
        (8.2, 8.8),
        (12.0, 13.0),
    ] {
        candidates.push(interval(&[(-a, 1.0), (b, -1.0), (1.0, 0.0)], 0.0, 20.0));
    }
    candidates.push(interval(&[(-1.0, 0.0)], 0.0, 20.0));
    let union = admitted(&candidates).expect("raw count above8 does not force marching");
    assert_eq!(union, vec![(1.0, 5.0), (8.0, 9.0), (12.0, 13.0)]);
    assert!((visible(&union, 0.0, 20.0) - reference_visible(&candidates, 0.0, 20.0)).abs() < 1e-12);
}
#[test]
fn eight_disjoint_intersections_fit_and_ninth_falls_back_without_omitting_a_blocker() {
    let mut candidates = vec![None; 12];
    candidates.extend((0..8).map(|i| Some((i as f32 * 2.0, i as f32 * 2.0 + 1.0))));
    let union = admitted(&candidates).unwrap();
    assert_eq!(union.len(), 8);
    assert!((visible(&union, 0.0, 20.0) - reference_visible(&candidates, 0.0, 20.0)).abs() < 1e-12);
    candidates.push(Some((18.0, 19.0)));
    assert!(admitted(&candidates).is_none());
    // A late bridge could make the final union fit, but keeping the full march is conservative.
    candidates.push(Some((0.0, 19.0)));
    assert!(admitted(&candidates).is_none());
}
#[test]
fn bounded_union_matches_independent_coverage_for_candidate_order_and_nested_shadows() {
    for count in 0..64 {
        let candidates: Vec<_> = (0..count)
            .map(|i| {
                let a = ((i * 17 + count * 3) % 31) as f32 * 0.5;
                Some((a, (a + ((i % 7) + 1) as f32).min(20.0)))
            })
            .collect();
        for reversed in [false, true] {
            let mut ordered = candidates.clone();
            if reversed {
                ordered.reverse();
            }
            if let Some(union) = admitted(&ordered) {
                assert!(union.len() <= 8 && union.windows(2).all(|p| p[0].1 < p[1].0));
                assert!(
                    (visible(&union, 0.0, 20.0) - reference_visible(&ordered, 0.0, 20.0)).abs()
                        < 1e-12
                );
            }
        }
    }
}

// CPU admission model, with an independent endpoint-integral reference below. Visits prove that
// complete opaque coverage needs no inspection of remaining blockers; gaps remain ordinary work.
fn early_visible(candidates: &[Option<(f32, f32)>], first: f32, last: f32) -> (Option<f64>, usize) {
    for end in 1..=candidates.len() {
        let Some(union) = admitted(&candidates[..end]) else {
            return (None, end);
        };
        if union.iter().any(|&(a, b)| a <= first && b >= last) {
            return (Some(0.0), end);
        }
    }
    (
        admitted(candidates).map(|union| visible(&union, first, last)),
        candidates.len(),
    )
}
#[test]
fn complete_opaque_ray_exits_before_seventy_later_candidates() {
    let mut candidates = vec![Some((0.0, 20.0))];
    candidates.extend((0..70).map(|i| Some((i as f32 * 0.1, i as f32 * 0.1 + 0.05))));
    assert_eq!(early_visible(&candidates, 0.0, 20.0), (Some(0.0), 1));
    assert_eq!(reference_visible(&candidates, 0.0, 20.0), 0.0);
}
#[test]
fn touching_or_overlapping_opaque_intervals_exit_only_once_the_ray_is_covered() {
    for second in [Some((10.0, 20.0)), Some((9.0, 20.0))] {
        let candidates = [Some((0.0, 10.0)), None, second, Some((2.0, 3.0))];
        assert_eq!(early_visible(&candidates, 0.0, 20.0), (Some(0.0), 3));
        assert_eq!(reference_visible(&candidates, 0.0, 20.0), 0.0);
    }
}
#[test]
fn even_one_representable_positive_gap_does_not_trigger_opaque_early_exit() {
    let after_ten = f32::from_bits(10.0_f32.to_bits() + 1);
    for gap_end in [after_ten, 10.001, 11.0] {
        let candidates = [Some((0.0, 10.0)), Some((gap_end, 20.0)), None];
        let (actual, visits) = early_visible(&candidates, 0.0, 20.0);
        let reference = reference_visible(&candidates, 0.0, 20.0);
        assert_eq!(visits, candidates.len());
        assert!(
            reference > 0.0 && actual.unwrap() > 0.0,
            "positive gap was lost"
        );
        assert!((actual.unwrap() - reference).abs() < 1e-12);
    }
}
