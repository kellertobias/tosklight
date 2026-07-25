#[test]
fn alignment_uses_shortest_wrapped_path_and_deterministic_order() {
    assert!((aligned_normalized("left", 1, 3, 0.9, 0.1, false).unwrap() - 0.5).abs() < 0.001);
    let wrapped = aligned_normalized("left", 1, 3, 0.9, 0.1, true).unwrap();
    assert!(!(0.001..=0.999).contains(&wrapped));
    assert!((aligned_normalized("right", 0, 3, 0.2, 0.8, false).unwrap() - 0.8).abs() < 0.001);
}
