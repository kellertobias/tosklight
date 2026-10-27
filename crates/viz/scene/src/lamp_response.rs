//! Visual lamp inertia, separate from cue/DMX targets and observer persistence.
//! Nominal parameters are a visual approximation, not measured fixture specifications.
use crate::{Scene, SceneValues};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LampResponseProfile {
    #[default]
    Instant,
    TungstenNominal,
}

const KNEE: f64 = 0.4;
const RISE_SECONDS: f64 = 0.025;
const HIGH_FALL_SECONDS: f64 = 0.045;
const LOW_FALL_SECONDS: f64 = 0.12;
const SETTLED_EPSILON: f64 = 0.0001;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Channel {
    output: f64,
    target: f64,
}

impl Channel {
    fn new(target: f32) -> Self {
        let target = level(target);
        Self {
            output: target,
            target,
        }
    }
    fn advance(&mut self, target: f32, elapsed: f64) {
        self.target = level(target);
        if elapsed <= 0.0 {
            return;
        }
        let from = self.output;
        if from < self.target {
            self.output = approach(from, self.target, elapsed, RISE_SECONDS);
        } else if from > KNEE && self.target < KNEE {
            // Solve the exact knee crossing rather than choosing one tau per display frame.
            let crossing = HIGH_FALL_SECONDS * ((from - self.target) / (KNEE - self.target)).ln();
            self.output = if elapsed <= crossing {
                approach(from, self.target, elapsed, HIGH_FALL_SECONDS)
            } else {
                approach(KNEE, self.target, elapsed - crossing, LOW_FALL_SECONDS)
            };
        } else {
            let tau = if from > KNEE {
                HIGH_FALL_SECONDS
            } else {
                LOW_FALL_SECONDS
            };
            self.output = approach(from, self.target, elapsed, tau);
        }
        if (self.output - self.target).abs() <= SETTLED_EPSILON {
            self.output = self.target;
        }
    }
}
fn level(value: f32) -> f64 {
    if value.is_finite() {
        f64::from(value.clamp(0.0, 1.0))
    } else {
        0.0
    }
}
fn approach(from: f64, target: f64, elapsed: f64, tau: f64) -> f64 {
    target + (from - target) * (-elapsed / tau).exp()
}

/// Finite state per current emitter/cell. Capture clones outputs, never a running clock.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LampResponseState {
    instance_id: Uuid,
    head_index: u16,
    emitter_id: Option<Uuid>,
    master: Channel,
    cells: Vec<Channel>,
}
impl LampResponseState {
    pub fn output(&self) -> f32 {
        self.master.output as f32
    }
    pub fn cell_output(&self, index: usize) -> Option<f32> {
        self.cells.get(index).map(|cell| cell.output as f32)
    }
    pub fn is_moving(&self) -> bool {
        self.master.output != self.master.target
            || self.cells.iter().any(|cell| cell.output != cell.target)
    }
    /// Thermal motion behind a closed shutter or a currently black recipe remains CPU state.
    /// Include the target, not only current output: the first instant of warm-up can be zero.
    pub fn can_change_picture(&self, value: &crate::EmitterValues) -> bool {
        value.shutter > 0.0
            && ((value.colour.iter().any(|channel| *channel > 0.0)
                && self.master.output != self.master.target)
                || self.cells.iter().zip(&value.cells).any(|(state, cell)| {
                    cell.colour.iter().any(|channel| *channel > 0.0) && state.output != state.target
                }))
    }
}

impl SceneValues {
    /// New scenes/unknown histories start at received equilibrium. Subsequent edges heat/cool;
    /// no unseen pre-attachment history is invented. Decoded targets are never modified.
    pub fn advance_lamp_response(&mut self, scene: &Scene, elapsed_seconds: f64) -> bool {
        let elapsed = if elapsed_seconds.is_finite() {
            elapsed_seconds.max(0.0)
        } else {
            0.0
        };
        let mut changed = false;
        for (index, value) in self.emitters.iter_mut().enumerate() {
            let before_scalar = value.retained_visible_intensity();
            let fixture = scene
                .emitters
                .get(index)
                .and_then(|emitter| scene.fixtures.get(emitter.fixture_index as usize));
            let identity = fixture.zip(scene.emitters.get(index));
            let emitter_id = scene.emitter_ids.get(index).copied();
            let mismatched = value.lamp_response.as_ref().is_some_and(|state| {
                !identity.is_some_and(|(fixture, emitter)| {
                    state.instance_id == fixture.instance_id
                        && state.head_index == emitter.head_index
                        && state.emitter_id == emitter_id
                })
            });
            let nominal = identity.is_some_and(|(fixture, emitter)| {
                fixture.lamp_response == LampResponseProfile::TungstenNominal
                    && matches!(
                        emitter.kind,
                        crate::EmitterKind::Beam | crate::EmitterKind::Emissive
                    )
            });
            // Rare structural/profile transitions may drop cell history. Snapshot only those
            // visible levels, never every CPU tick or an unbounded stream of raw frames.
            let old_cells = (mismatched || !nominal).then(|| {
                (0..value
                    .lamp_response
                    .as_ref()
                    .map_or(0, |state| state.cells.len()))
                    .map(|index| value.resolved_cell_intensity(index))
                    .collect::<Vec<_>>()
            });
            if mismatched {
                clear_history(value);
            }
            if !nominal {
                if value.lamp_response.is_some() {
                    clear_history(value);
                }
                changed |= before_scalar != value.retained_visible_intensity()
                    || cells_changed(value, old_cells.as_deref());
                continue;
            }
            let (fixture, emitter) = identity.unwrap();
            let cell_count = value.cells.len().min(emitter.cells.len());
            if value.lamp_response.is_none() {
                value.lamp_response = Some(LampResponseState {
                    instance_id: fixture.instance_id,
                    head_index: emitter.head_index,
                    emitter_id,
                    master: Channel::new(value.intensity),
                    cells: value
                        .cells
                        .iter()
                        .take(cell_count)
                        .map(|cell| Channel::new(cell.intensity))
                        .collect(),
                });
            }
            let state = value.lamp_response.as_mut().unwrap();
            state.master.advance(value.intensity, elapsed);
            state.cells.truncate(cell_count);
            for (index, cell) in value.cells.iter().take(cell_count).enumerate() {
                if index == state.cells.len() {
                    state.cells.push(Channel::new(cell.intensity));
                }
                let before = state.cells[index].output;
                state.cells[index].advance(cell.intensity, elapsed);
                if value.shutter > 0.0 && cell.colour.iter().any(|channel| *channel > 0.0) {
                    changed |= (before as f32 * value.shutter).clamp(0.0, 1.0)
                        != (state.cells[index].output as f32 * value.shutter).clamp(0.0, 1.0);
                }
            }
            changed |= before_scalar != value.retained_visible_intensity()
                || cells_changed(value, old_cells.as_deref());
        }
        changed
    }
    pub fn retain_lamp_response_from(&mut self, previous: &Self) {
        for (next, previous) in self.emitters.iter_mut().zip(&previous.emitters) {
            next.lamp_response = previous.lamp_response.clone();
            if previous.lamp_response.is_some() {
                next.held_intensity = previous.held_intensity;
                for (next, previous) in next.cells.iter_mut().zip(&previous.cells) {
                    next.held_intensity = previous.held_intensity;
                }
            }
        }
    }
    /// A provider/show reset or explicit seek drops history instead of replaying an old tail.
    pub fn reset_lamp_response(&mut self) {
        for value in &mut self.emitters {
            if value.lamp_response.is_some() {
                clear_history(value);
            }
        }
    }
}

fn clear_history(value: &mut crate::EmitterValues) {
    value.lamp_response = None;
    value.held_intensity = 0.0;
    for cell in &mut value.cells {
        cell.held_intensity = 0.0;
    }
}

fn cells_changed(value: &crate::EmitterValues, before: Option<&[f32]>) -> bool {
    before.is_some_and(|levels| {
        levels
            .iter()
            .enumerate()
            .any(|(index, previous)| *previous != value.resolved_cell_intensity(index))
    })
}

#[cfg(test)]
mod tests;
