use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaserMode {
    Absolute,
    Relative,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaserCurve {
    Step,
    Linear,
    Sine,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaserStep {
    pub position: f32,
    pub value: f32,
    pub curve_to_next: PhaserCurve,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phaser {
    pub mode: PhaserMode,
    pub steps: Vec<PhaserStep>,
    pub cycles_per_minute: f32,
    pub phase_start_degrees: f32,
    pub phase_end_degrees: f32,
    pub width: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributePhaser {
    pub fixture_ids: Vec<FixtureId>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    pub attribute: AttributeKey,
    pub phaser: Phaser,
}

impl Phaser {
    pub fn validate(&self) -> Result<(), String> {
        if self.steps.is_empty() {
            return Err("a phaser needs at least one step".into());
        }
        if !self.cycles_per_minute.is_finite() || self.cycles_per_minute <= 0.0 {
            return Err("phaser speed must be positive".into());
        }
        if !(0.0..=1.0).contains(&self.width) || self.width == 0.0 {
            return Err("phaser width must be within (0,1]".into());
        }
        let mut previous = -1.0;
        for step in &self.steps {
            if !(0.0..1.0).contains(&step.position)
                || step.position <= previous
                || !step.value.is_finite()
            {
                return Err("phaser steps must be finite and strictly ordered within [0,1)".into());
            }
            previous = step.position;
        }
        Ok(())
    }

    pub fn sample(&self, elapsed_seconds: f64, fixture_index: usize, fixture_count: usize) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let spread = if fixture_count <= 1 {
            0.0
        } else {
            fixture_index as f32 / (fixture_count - 1) as f32
        };
        let degrees =
            self.phase_start_degrees + (self.phase_end_degrees - self.phase_start_degrees) * spread;
        let mut phase = ((elapsed_seconds * f64::from(self.cycles_per_minute) / 60.0) as f32
            + degrees / 360.0)
            .rem_euclid(1.0);
        if phase > self.width {
            phase = 0.0;
        } else {
            phase /= self.width;
        }
        let current_index = self
            .steps
            .iter()
            .rposition(|step| step.position <= phase)
            .unwrap_or(self.steps.len() - 1);
        let current = &self.steps[current_index];
        let next = &self.steps[(current_index + 1) % self.steps.len()];
        let span = if next.position > current.position {
            next.position - current.position
        } else {
            1.0 - current.position + next.position
        };
        let distance = if phase >= current.position {
            phase - current.position
        } else {
            1.0 - current.position + phase
        };
        let mut progress = if span > 0.0 {
            (distance / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        progress = match current.curve_to_next {
            PhaserCurve::Step => 0.0,
            PhaserCurve::Linear => progress,
            PhaserCurve::Sine => (1.0 - (std::f32::consts::PI * progress).cos()) * 0.5,
        };
        current.value + (next.value - current.value) * progress
    }
}
