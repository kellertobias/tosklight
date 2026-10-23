use super::layout::BaselineOptions;
use viz_helper::protocol::{RenderQuality, StageViewMode, ToHelper};

pub(super) fn apply(message: &mut ToHelper, options: &BaselineOptions) {
    let ToHelper::Picture {
        quality,
        mode,
        atmosphere,
        ..
    } = message
    else {
        return;
    };
    if options.baseline_layout.is_none() {
        return;
    }
    *quality = match options.baseline_quality.as_str() {
        "standard" => RenderQuality::Standard,
        "high" => RenderQuality::High,
        "ultra" => RenderQuality::Ultra,
        _ => RenderQuality::Draft,
    };
    *mode = if options.baseline_view == "full_3d" {
        StageViewMode::Full3d
    } else {
        StageViewMode::Lines3d
    };
    // Control suppresses haze for its diagram. The full-render baseline uses a declared fixed look.
    *atmosphere = if options.baseline_view == "full_3d" {
        0.35
    } else {
        0.0
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicitly_configured_fixed_baseline_can_measure_actual_full_rendering() {
        let mut picture = ToHelper::Picture {
            atmosphere: 0.0,
            ambient: 0.35,
            quality: RenderQuality::Draft,
            exposure: 1.0,
            laser_brightness: 1.0,
            lamp_fog_cloudiness: 0.0,
            lamp_fog_turbulence: 0.0,
            laser_fog_cloudiness: 0.0,
            laser_fog_turbulence: 0.0,
            show_labels: true,
            floor_grid: true,
            show_beam_guides: true,
            background: [0.0; 3],
            mode: StageViewMode::Lines3d,
            follow_preload: true,
        };
        let options = BaselineOptions {
            baseline_layout: Some("single".into()),
            baseline_quality: "high".into(),
            baseline_view: "full_3d".into(),
        };
        apply(&mut picture, &options);
        assert!(matches!(
            picture,
            ToHelper::Picture {
                quality: RenderQuality::High,
                mode: StageViewMode::Full3d,
                atmosphere: 0.35,
                follow_preload: true,
                ..
            }
        ));
        let normal = BaselineOptions {
            baseline_layout: None,
            baseline_quality: "draft".into(),
            baseline_view: "lines_3d".into(),
        };
        apply(&mut picture, &normal);
        assert!(matches!(
            picture,
            ToHelper::Picture {
                quality: RenderQuality::High,
                mode: StageViewMode::Full3d,
                ..
            }
        ));
    }
}
