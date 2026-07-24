use super::*;

#[derive(Default, Deserialize)]
pub(super) struct PoolPlaybackInput {
    pub(super) value: Option<f32>,
    pub(super) cue_number: Option<f64>,
    pub(super) pressed: Option<bool>,
    pub(super) button: Option<u8>,
}
