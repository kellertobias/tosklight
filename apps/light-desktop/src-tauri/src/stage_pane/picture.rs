//! Control Stage is a diagram, never the full visualizer. Enforce that contract at the native
//! boundary as well as the UI, including requests restored from older surface configurations.

use viz_helper::protocol::{RenderQuality, StageViewMode, ToHelper};

/// The operator's picture settings for the pane, which belong to the renderer drawing it.
///
/// Named as the web layer names them, for the reason [`super::PaneGeometry`] is.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Picture {
    atmosphere: f32,
    ambient: f32,
    exposure: f32,
    laser_brightness: f32,
    lamp_fog_cloudiness: f32,
    lamp_fog_turbulence: f32,
    laser_fog_cloudiness: f32,
    laser_fog_turbulence: f32,
    #[serde(default)]
    detailed_people: bool,
    show_labels: bool,
    floor_grid: bool,
    show_beam_guides: bool,
    /// The colour behind the rig, linear RGB.
    background: [f32; 3],
    /// Which way the Stage is being looked at, as the web layer names it.
    mode: String,
    follow_preload: bool,
}

impl Picture {
    /// Sent before Embed so even the first pane frame is a cheap diagram. The current operator
    /// settings follow normally; these defaults only cover the startup handshake.
    pub(super) fn startup() -> Self {
        Self {
            ambient: 0.35,
            exposure: 1.0,
            show_labels: true,
            floor_grid: true,
            ..Self::default()
        }
    }

    pub(super) fn into_helper(self) -> ToHelper {
        ToHelper::Picture {
            atmosphere: self.atmosphere,
            ambient: self.ambient,
            quality: RenderQuality::Draft,
            exposure: self.exposure,
            laser_brightness: self.laser_brightness,
            lamp_fog_cloudiness: self.lamp_fog_cloudiness,
            lamp_fog_turbulence: self.lamp_fog_turbulence,
            laser_fog_cloudiness: self.laser_fog_cloudiness,
            laser_fog_turbulence: self.laser_fog_turbulence,
            detailed_people: self.detailed_people,
            show_labels: self.show_labels,
            floor_grid: self.floor_grid,
            show_beam_guides: self.show_beam_guides,
            background: self.background,
            mode: match self.mode.as_str() {
                "top_down" => StageViewMode::TopDown,
                "left_to_right" => StageViewMode::LeftToRight,
                "right_to_left" => StageViewMode::RightToLeft,
                "front_to_back" => StageViewMode::FrontToBack,
                "back_to_front" => StageViewMode::BackToFront,
                _ => StageViewMode::Lines3d,
            },
            follow_preload: self.follow_preload,
        }
    }
}

impl super::StagePane {
    /// Send the operator's picture settings on to the renderer, which owns this picture.
    pub(crate) fn send_picture(&self, picture: Picture) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            let mut instruction = picture.into_helper();
            crate::packaged_benchmark::baseline_picture(&mut instruction)?;
            running.send(&instruction);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_picture(picture: Picture, expected_mode: StageViewMode) {
        let ToHelper::Picture {
            mode,
            quality,
            follow_preload,
            ..
        } = picture.into_helper()
        else {
            panic!("expected native picture instruction");
        };
        assert_eq!(mode, expected_mode);
        assert_eq!(quality, RenderQuality::Draft);
        assert!(follow_preload);
    }

    #[test]
    fn optional_detail_defaults_off_but_survives_the_lightweight_live_picture() {
        let mut payload = serde_json::json!({
            "atmosphere": 0.0, "ambient": 0.35, "exposure": 1.0, "laserBrightness": 1.0,
            "lampFogCloudiness": 0.0, "lampFogTurbulence": 0.0,
            "laserFogCloudiness": 0.0, "laserFogTurbulence": 0.0,
            "showLabels": true, "floorGrid": true, "showBeamGuides": false,
            "background": [0.0,0.0,0.0], "mode": "lines_3d", "followPreload": false
        });
        for expected in [false, true] {
            if expected {
                payload["detailedPeople"] = serde_json::Value::Bool(true);
            }
            let picture: Picture = serde_json::from_value(payload.clone()).unwrap();
            let ToHelper::Picture {
                detailed_people,
                quality,
                mode,
                ..
            } = picture.into_helper()
            else {
                panic!("expected picture");
            };
            assert_eq!(detailed_people, expected);
            assert_eq!(quality, RenderQuality::Draft);
            assert_eq!(mode, StageViewMode::Lines3d);
        }
    }

    #[test]
    fn legacy_render_modes_and_unknown_modes_are_draft_wireframes() {
        for mode in ["full_3d", "simple_3d", "lines_3d", "", "future_render_mode"] {
            for quality in ["draft", "standard", "high", "ultra"] {
                let payload = serde_json::json!({
                    "atmosphere": 0.5, "ambient": 0.4, "quality": quality,
                    "exposure": 1.0, "laserBrightness": 1.0,
                    "lampFogCloudiness": 0.0, "lampFogTurbulence": 0.0,
                    "laserFogCloudiness": 0.0, "laserFogTurbulence": 0.0,
                    "showLabels": true, "floorGrid": true, "showBeamGuides": false,
                    "background": [0.0,0.0,0.0], "mode": mode, "followPreload": true
                });
                let picture: Picture = serde_json::from_value(payload).unwrap();
                assert_picture(picture, StageViewMode::Lines3d);
            }
        }
    }

    #[test]
    fn each_orthographic_control_plan_keeps_its_direction() {
        for (mode, expected) in [
            ("top_down", StageViewMode::TopDown),
            ("left_to_right", StageViewMode::LeftToRight),
            ("right_to_left", StageViewMode::RightToLeft),
            ("front_to_back", StageViewMode::FrontToBack),
            ("back_to_front", StageViewMode::BackToFront),
        ] {
            assert_picture(
                Picture {
                    mode: mode.into(),
                    follow_preload: true,
                    ..Picture::startup()
                },
                expected,
            );
        }
    }

    #[test]
    fn startup_picture_cannot_request_full_rendering() {
        let ToHelper::Picture {
            mode,
            quality,
            exposure,
            ..
        } = Picture::startup().into_helper()
        else {
            panic!("expected native startup picture");
        };
        assert_eq!(mode, StageViewMode::Lines3d);
        assert_eq!(quality, RenderQuality::Draft);
        assert!(exposure > 0.0);
    }
}
