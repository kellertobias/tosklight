use std::collections::HashSet;

use super::{TimecodeDefinition, TimecodeFrame, TimecodeLaneContent};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimecodeValidationError {
    pub message: String,
}

impl TimecodeValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl TimecodeDefinition {
    /// Validates portable invariants before an active-show mutation is committed.
    pub fn validate(&self) -> Result<(), TimecodeValidationError> {
        if self.id.0.is_nil() {
            return Err(TimecodeValidationError::new("Timecode id must not be nil"));
        }
        if self.number == 0 {
            return Err(TimecodeValidationError::new(
                "Timecode number must be positive",
            ));
        }
        if self.duration.is_none() && self.audio.is_none() {
            return Err(TimecodeValidationError::new(
                "a Timecode without audio requires a configured duration",
            ));
        }
        if self.duration.is_some_and(|duration| duration.0 == 0) {
            return Err(TimecodeValidationError::new(
                "configured Timecode duration must be positive",
            ));
        }
        if self
            .audio
            .as_ref()
            .is_some_and(|audio| audio.asset_id.is_nil())
        {
            return Err(TimecodeValidationError::new(
                "Timecode audio asset id must not be nil",
            ));
        }

        let mut identities = HashSet::new();
        for marker in &self.markers {
            unique(&mut identities, marker.id.0, "marker")?;
            within_duration(marker.frame, self.duration, "marker")?;
        }

        for lane in &self.lanes {
            unique(&mut identities, lane.id.0, "lane")?;
            match &lane.content {
                TimecodeLaneContent::CueList { clips, .. } => {
                    let mut previous = TimecodeFrame::ZERO;
                    let mut previous_end = None;
                    for clip in clips {
                        unique(&mut identities, clip.id.0, "Cuelist clip")?;
                        ordered(previous, clip.start_frame, "Cuelist clips")?;
                        if previous_end.is_some_and(|end: TimecodeFrame| clip.start_frame.0 < end.0)
                        {
                            return Err(TimecodeValidationError::new(
                                "Cuelist clips on one lane must not overlap",
                            ));
                        }
                        if clip.end_frame.0 < clip.start_frame.0 {
                            return Err(TimecodeValidationError::new(
                                "Cuelist clip end precedes its start",
                            ));
                        }
                        if clip.start_cue_id.is_nil() || clip.end_cue_id.is_nil() {
                            return Err(TimecodeValidationError::new(
                                "Cuelist clip Cue ids must not be nil",
                            ));
                        }
                        within_duration(clip.end_frame, self.duration, "Cuelist clip")?;
                        previous = clip.start_frame;
                        previous_end = Some(clip.end_frame);
                    }
                }
                TimecodeLaneContent::SpeedGroup { group, keyframes } => {
                    if group.trim().is_empty() {
                        return Err(TimecodeValidationError::new(
                            "Speed Group lane requires a group",
                        ));
                    }
                    let mut previous = TimecodeFrame::ZERO;
                    for keyframe in keyframes {
                        unique(&mut identities, keyframe.id.0, "Speed Group keyframe")?;
                        ordered(previous, keyframe.frame, "Speed Group keyframes")?;
                        within_duration(keyframe.frame, self.duration, "Speed Group keyframe")?;
                        if !keyframe.bpm.is_finite() || keyframe.bpm <= 0.0 {
                            return Err(TimecodeValidationError::new(
                                "Speed Group BPM must be finite and positive",
                            ));
                        }
                        if !keyframe.phase.is_finite() || !(0.0..1.0).contains(&keyframe.phase) {
                            return Err(TimecodeValidationError::new(
                                "Speed Group phase must be within 0 inclusive and 1 exclusive",
                            ));
                        }
                        previous = keyframe.frame;
                    }
                }
                TimecodeLaneContent::AudioVolume { keyframes } => {
                    let mut previous = TimecodeFrame::ZERO;
                    for keyframe in keyframes {
                        unique(&mut identities, keyframe.id.0, "audio-volume keyframe")?;
                        ordered(previous, keyframe.frame, "audio-volume keyframes")?;
                        within_duration(keyframe.frame, self.duration, "audio-volume keyframe")?;
                        if !keyframe.value.is_finite() || !(0.0..=1.0).contains(&keyframe.value) {
                            return Err(TimecodeValidationError::new(
                                "audio volume must be within 0 and 1",
                            ));
                        }
                        previous = keyframe.frame;
                    }
                }
            }
        }
        Ok(())
    }
}

fn unique(
    identities: &mut HashSet<uuid::Uuid>,
    id: uuid::Uuid,
    label: &str,
) -> Result<(), TimecodeValidationError> {
    if id.is_nil() || !identities.insert(id) {
        Err(TimecodeValidationError::new(format!(
            "{label} id must be unique and non-nil"
        )))
    } else {
        Ok(())
    }
}

fn ordered(
    previous: TimecodeFrame,
    current: TimecodeFrame,
    label: &str,
) -> Result<(), TimecodeValidationError> {
    if current.0 < previous.0 {
        Err(TimecodeValidationError::new(format!(
            "{label} must remain in persisted frame order"
        )))
    } else {
        Ok(())
    }
}

fn within_duration(
    frame: TimecodeFrame,
    duration: Option<TimecodeFrame>,
    label: &str,
) -> Result<(), TimecodeValidationError> {
    if duration.is_some_and(|duration| frame.0 > duration.0) {
        Err(TimecodeValidationError::new(format!(
            "{label} lies after the configured Timecode duration"
        )))
    } else {
        Ok(())
    }
}
