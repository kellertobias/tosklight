//! Direct edits predict from their immutable ORIGINAL fixture, never the patched destination.
use super::{
    ChannelFunctionBehavior, FixtureMode, FixtureProfile, NativeColorBinding, NativeColorIdentity,
    PhysicalDataQuality, ProfileError, native_color_function_allowed,
};
use crate::forward::{ColorForwardFlags, ColorForwardResult, CompiledColorForward};
use light_core::programming::{
    IntentError, NativeColorComponentDescriptor, NativeColorEditModel, NativeColorPrediction,
    NativeColorRecipe, NativeDriveLimit, PortableColorEstimate, PortableUv, PortableVisibleColor,
};
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Copy, Debug)]
struct Control {
    raw_index: usize,
    seen_index: usize,
    descriptor: NativeColorComponentDescriptor,
}

#[derive(Debug)]
struct PredictionScratch {
    raw: Vec<u32>,
    seen: Vec<bool>,
    output: Vec<ColorForwardResult>,
}

#[derive(Debug)]
enum NativePrediction {
    Forward(CompiledColorForward),
    Unavailable(String),
}

/// Build once per original source identity at configuration time. The compiled tables and
/// reusable scratch avoid profile hashing, spectrum resampling and native-buffer allocation
/// during prediction. Different destinations can share this source; no installed calibration
/// or destination channel layout participates in its portable estimate.
#[derive(Debug)]
pub struct CompiledNativeColorEditModel {
    source: NativeColorIdentity,
    controls: HashMap<NativeColorBinding, Control>,
    control_count: usize,
    prediction: NativePrediction,
    scratch: Mutex<PredictionScratch>,
}

impl CompiledNativeColorEditModel {
    pub fn compile(
        profile: &FixtureProfile,
        source: &NativeColorIdentity,
    ) -> Result<Self, ProfileError> {
        let invalid = |message: &str| ProfileError::Invalid(message.into());
        // Verify before any runtime compaction: native layout equality alone cannot authorize
        // re-predicting an old recipe from a newer optical model.
        if profile.native_color_identity(source.mode_id, source.head_id)? != *source {
            return Err(invalid(
                "native edit model requires its exact original profile identity",
            ));
        }
        // A derived model is read from the same projection its identity names.
        let profile = profile
            .native_color_source(source.mode_id)
            .expect("verified source mode");
        let mode = profile.mode(source.mode_id).expect("verified source mode");
        super::forward::validate_native_domains(mode)?;
        let forward = CompiledColorForward::compile(&profile, mode.id, None)
            .map_err(|error| error.to_string())
            .and_then(|model| model.ok_or_else(|| "native source has no optical prediction".into()))
            .and_then(|model| {
                model
                    .for_head(source.head_id)
                    .ok_or_else(|| "native source head has no optical prediction".into())
            });
        Self::compile_path(mode, source, forward)
    }

    /// Compile a retained multi-head mode once. One head's recordability limit must not discard
    /// sibling heads. Invalid native domains fail; unavailable optical prediction remains a
    /// native-only model with passive limitations.
    pub fn compile_mode(
        profile: &FixtureProfile,
        mode_id: uuid::Uuid,
    ) -> Result<Vec<(NativeColorIdentity, Result<Self, ProfileError>)>, ProfileError> {
        let identities = profile.native_color_identities(mode_id)?;
        // A derived model is read from the same projection its identities name.
        let profile = profile
            .native_color_source(mode_id)
            .expect("verified source mode");
        let mode = profile.mode(mode_id).expect("verified source mode");
        super::forward::validate_native_domains(mode)?;
        let forward = CompiledColorForward::compile(&profile, mode_id, None)
            .map_err(|error| error.to_string())
            .and_then(|model| {
                model.ok_or_else(|| "native source has no optical prediction".into())
            });
        Ok(identities
            .into_iter()
            .map(|source| {
                let prediction = forward.as_ref().map_err(Clone::clone).and_then(|model| {
                    model
                        .for_head(source.head_id)
                        .ok_or_else(|| "native source head has no optical prediction".into())
                });
                let model = Self::compile_path(mode, &source, prediction);
                (source, model)
            })
            .collect())
    }

    fn compile_path(
        mode: &FixtureMode,
        source: &NativeColorIdentity,
        forward: Result<CompiledColorForward, String>,
    ) -> Result<Self, ProfileError> {
        let invalid = |message: &str| ProfileError::Invalid(message.into());
        let physical = mode
            .color_physical
            .as_ref()
            .expect("verified physical source");
        let path_index = physical
            .paths
            .iter()
            .position(|p| p.id == source.path_id)
            .ok_or_else(|| invalid("native edit source path is missing"))?;
        let path = &physical.paths[path_index];
        if !(1..=512).contains(&path.controls.len()) {
            return Err(invalid("recordable native color requires 1-512 controls"));
        }
        let mut controls = HashMap::new();
        for (seen_index, id) in path.controls.iter().enumerate() {
            let (raw_index, channel) = mode
                .channels
                .iter()
                .enumerate()
                .find(|(_, c)| c.id == *id)
                .expect("validated path control");
            for function in &channel.functions {
                if native_color_function_allowed(channel, function) {
                    let binding = NativeColorBinding {
                        channel_id: *id,
                        function_id: function.id,
                    };
                    controls.insert(
                        binding,
                        Control {
                            raw_index,
                            seen_index,
                            descriptor: NativeColorComponentDescriptor {
                                binding,
                                raw_from: function.dmx_from,
                                raw_to: function.dmx_to,
                                continuous: matches!(
                                    function.behavior,
                                    ChannelFunctionBehavior::Continuous { .. }
                                ),
                            },
                        },
                    );
                }
            }
        }
        let scratch = PredictionScratch {
            raw: if forward.is_ok() {
                mode.channels.iter().map(|c| c.default_raw).collect()
            } else {
                Vec::new()
            },
            seen: vec![false; path.controls.len()],
            output: forward
                .as_ref()
                .map_or_else(|_| Vec::new(), |model| model.create_output()),
        };
        Ok(Self {
            source: source.clone(),
            controls,
            control_count: path.controls.len(),
            prediction: match forward {
                Ok(model) => NativePrediction::Forward(model),
                Err(reason) => NativePrediction::Unavailable(reason),
            },
            scratch: Mutex::new(scratch),
        })
    }
}

impl NativeColorEditModel for CompiledNativeColorEditModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }

    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        self.controls.get(&binding).map(|c| c.descriptor)
    }

    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.evaluate(recipe).map(|prediction| prediction.portable)
    }

    fn predict_with_status(
        &self,
        recipe: &NativeColorRecipe,
    ) -> Result<NativeColorPrediction, IntentError> {
        self.evaluate(recipe)
    }
}

impl CompiledNativeColorEditModel {
    /// One validated source evaluation. The drive diagnostic never edits the exact recipe.
    fn evaluate(&self, recipe: &NativeColorRecipe) -> Result<NativeColorPrediction, IntentError> {
        let invalid = |message: &str| IntentError(message.into());
        if recipe.source != self.source {
            return Err(invalid(
                "native prediction requires the original recipe source",
            ));
        }
        if !recipe.spreads.is_empty() {
            return Err(invalid("resolve native color spreads before prediction"));
        }
        if recipe.channels.len() != self.control_count {
            return Err(invalid(
                "native prediction requires every original Color control",
            ));
        }
        let mut scratch = self
            .scratch
            .lock()
            .map_err(|_| invalid("native prediction workspace is unavailable"))?;
        scratch.seen.fill(false);
        for value in &recipe.channels {
            let binding = NativeColorBinding {
                channel_id: value.channel_id,
                function_id: value.function_id,
            };
            let control = self
                .controls
                .get(&binding)
                .ok_or_else(|| invalid("native recipe references an unavailable Color function"))?;
            if scratch.seen[control.seen_index] {
                return Err(invalid("native recipe repeats a Color control"));
            }
            let domain = control.descriptor.raw_from..=control.descriptor.raw_to;
            if !domain.contains(&value.raw) {
                return Err(invalid("native recipe value is outside its function"));
            }
            scratch.seen[control.seen_index] = true;
            if matches!(self.prediction, NativePrediction::Forward(_)) {
                scratch.raw[control.raw_index] = value.raw;
            }
        }
        let NativePrediction::Forward(forward) = &self.prediction else {
            let NativePrediction::Unavailable(reason) = &self.prediction else {
                unreachable!()
            };
            return Ok(NativeColorPrediction {
                portable: PortableColorEstimate {
                    model_revision: self.source.model_revision,
                    visible: None,
                    uv: None,
                    quality: PhysicalDataQuality::Unknown,
                    limitations: vec![format!(
                        "Native controls remain available; optical prediction is unavailable: {}",
                        reason.chars().take(200).collect::<String>()
                    )],
                },
                drive_limit: NativeDriveLimit::Unknown,
            });
        };
        let PredictionScratch { raw, output, .. } = &mut *scratch;
        forward
            .evaluate(raw, output)
            .map_err(|_| invalid("native recipe does not match its compiled layout"))?;
        let result = &output[0];
        let estimate = PortableColorEstimate {
            model_revision: self.source.model_revision,
            // XYZ already includes drive response and known UV leakage. Multiplying by Y here
            // would apply intensity twice; normalization would lose known black/output.
            visible: result.visible_complete.then_some(PortableVisibleColor {
                xyz: result.known_xyz,
                relative_output: 1.0,
            }),
            uv: result.portable_uv.map(|uv| PortableUv {
                amount: uv.amount as f32,
                quality: uv.quality,
            }),
            quality: if result.visible_complete {
                result.data_quality
            } else {
                PhysicalDataQuality::Unknown
            },
            limitations: limitations(result),
        };
        estimate.validate()?;
        Ok(NativeColorPrediction {
            portable: estimate,
            drive_limit: if result.flags.contains(ColorForwardFlags::NATIVE_OVER_LIMIT) {
                NativeDriveLimit::AboveModelMaximum
            } else {
                NativeDriveLimit::Within
            },
        })
    }
}

fn limitations(result: &ColorForwardResult) -> Vec<String> {
    let mut notes = Vec::new();
    if !result.visible_complete {
        notes.push("Visible appearance is not fully known for this native recipe.".into());
    }
    if result.portable_uv.is_none() {
        notes.push("A portable UV amount is not known for this native recipe.".into());
    }
    if result.flags.contains(ColorForwardFlags::NATIVE_OVER_LIMIT) {
        notes.push("Native drive exceeds the source model's configured maximum.".into());
    }
    notes
}
