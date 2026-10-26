//! The fused shader uses attachment pixel coordinates; resampling uses the ordinary UV post pass.
pub(super) fn compatible(beams: bool, lasers: bool, scene: (u32, u32), output: (u32, u32)) -> bool {
    beams && !lasers && scene == output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_matching_attachment_extents_can_use_pixel_addressed_fused_composition() {
        assert!(compatible(true, false, (641, 361), (641, 361)));
        // Check either axis independently, including the odd-size rounding case.
        for scene in [(320, 180), (641, 360), (640, 361)] {
            assert!(!compatible(true, false, scene, (641, 361)));
        }
        assert!(!compatible(false, false, (641, 361), (641, 361)));
        assert!(!compatible(true, true, (641, 361), (641, 361)));
    }
}
