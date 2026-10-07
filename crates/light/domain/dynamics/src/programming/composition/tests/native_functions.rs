use super::*;
use std::cell::RefCell;

struct FunctionsModel {
    source: NativeColorIdentity,
}

fn binding(channel: usize, function: usize) -> NativeColorBinding {
    NativeColorBinding {
        channel_id: Uuid::from_u128(100 + channel as u128),
        function_id: Uuid::from_u128(200 + (channel * 2 + function) as u128),
    }
}

impl NativeColorEditModel for FunctionsModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }

    fn descriptor(&self, requested: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        for channel in 0..2 {
            for function in 0..2 {
                if requested == binding(channel, function) {
                    return Some(NativeColorComponentDescriptor {
                        binding: requested,
                        raw_from: if function == 0 { 0 } else { 256 },
                        raw_to: if function == 0 { 255 } else { u32::MAX },
                        continuous: true,
                    });
                }
            }
        }
        None
    }

    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        ensure(
            recipe.source == self.source && recipe.channels.len() == 2,
            "prediction requires this complete source",
        )?;
        for channel in 0..2 {
            let value = recipe
                .channels
                .iter()
                .find(|value| value.channel_id == binding(channel, 0).channel_id)
                .ok_or_else(|| IntentError("missing native channel".into()))?;
            let descriptor = self
                .descriptor(NativeColorBinding {
                    channel_id: value.channel_id,
                    function_id: value.function_id,
                })
                .ok_or_else(|| IntentError("unknown native function".into()))?;
            ensure(
                (descriptor.raw_from..=descriptor.raw_to).contains(&value.raw),
                "native function bounds",
            )?;
        }
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.37,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}

fn model() -> Arc<FunctionsModel> {
    let mut source = native_model().source.clone();
    source.profile_digest = "multi-function-source".into();
    source.native_layout_signature = "two-channels-two-functions".into();
    Arc::new(FunctionsModel { source })
}

fn complete(model: &FunctionsModel, functions: [usize; 2], raw: [u32; 2]) -> AttributeValue {
    let recipe = NativeColorRecipe {
        source: model.source.clone(),
        channels: (0..2)
            .map(|channel| NativeColorValue {
                channel_id: binding(channel, functions[channel]).channel_id,
                function_id: binding(channel, functions[channel]).function_id,
                raw: raw[channel],
            })
            .collect(),
        spreads: vec![],
    };
    let portable = model.predict(&recipe).unwrap();
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct { recipe, portable }))
}

fn recipe(value: &AttributeValue) -> &NativeColorRecipe {
    let AttributeValue::ColorProgram(color) = value else {
        panic!("Color owner")
    };
    let ColorProgram::Direct { recipe, .. } = color.as_ref() else {
        panic!("Direct source")
    };
    recipe
}

fn native(
    model: &Arc<FunctionsModel>,
    channel: usize,
    function: usize,
    raw: u32,
    rank: u128,
) -> FamilySample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::DirectColor {
                        source: model.source.clone(),
                    },
                    component: Some(ProgrammingComponent::NativeColor(binding(
                        channel, function,
                    ))),
                },
                Some(model.clone()),
            )
            .unwrap(),
        ),
        DynamicValue::Native(raw),
        color(ColorComponent::Red, 0.0, rank).rank,
        1.0,
    )
    .unwrap()
}

fn adopt(
    model: &FunctionsModel,
    value: &AttributeValue,
    address: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    let Some(ProgrammingComponent::NativeColor(binding)) = address.component else {
        panic!("native address")
    };
    let mut recipe = recipe(value).clone();
    ensure(
        recipe.source == model.source,
        "original source must stay pinned",
    )?;
    let descriptor = model.descriptor(binding).unwrap();
    let channel = recipe
        .channels
        .iter_mut()
        .find(|value| value.channel_id == binding.channel_id)
        .unwrap();
    channel.function_id = binding.function_id;
    channel.raw = descriptor.raw_from;
    let portable = model.predict(&recipe)?;
    Ok(AttributeValue::ColorProgram(Arc::new(
        ColorProgram::Direct { recipe, portable },
    )))
}

#[test]
fn each_native_function_takeover_receives_the_complete_already_composed_recipe() {
    let model = model();
    let observations = RefCell::new(Vec::new());
    let resolver = |value: &AttributeValue, address: &DynamicValueAddress| {
        observations.borrow_mut().push(recipe(value).clone());
        adopt(&model, value, address)
    };
    let stale_model = native_model();
    let base = complete(&model, [0, 0], [10, 20]);
    let samples = [
        native(&model, 0, 1, u32::MAX - 1, 1),
        native(&model, 1, 1, u32::MAX - 3, 2),
    ];
    let actual = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                native_model: Some(stale_model.as_ref()),
                ..Default::default()
            },
            resolve_adoption: Some(&resolver),
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(
        actual,
        complete(&model, [1, 1], [u32::MAX - 1, u32::MAX - 3])
    );
    let seen = observations.borrow();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], *recipe(&base));
    assert_eq!(
        seen[1],
        *recipe(&complete(&model, [1, 0], [u32::MAX - 1, 20]))
    );
}

#[test]
fn native_function_selection_is_per_channel_even_if_the_highest_lane_uses_another_channel() {
    let model = model();
    let base = complete(&model, [0, 0], [40, 20]);
    let mut selected = native(&model, 0, 1, 768, 2);
    selected.activation_mix = 0.5;
    let mut samples = [
        native(&model, 0, 0, 10, 1),
        selected,
        native(&model, 1, 0, 55, 3),
    ];
    for _ in 0..2 {
        let observations = RefCell::new(Vec::new());
        let resolver = |value: &AttributeValue, address: &DynamicValueAddress| {
            observations.borrow_mut().push(recipe(value).clone());
            adopt(&model, value, address)
        };
        let actual = compose_dynamic_family(
            ProgrammingOwner::Color,
            &base,
            &samples,
            &FamilyCompositionContext {
                resolve_adoption: Some(&resolver),
                ..Default::default()
            },
            &mut FamilyCompositionScratch::default(),
        )
        .unwrap();
        // The old function contributes neither raw 10 nor a fabricated numeric crossfade.
        // The selected function starts at its coherent adopted raw 256, then blends to 768.
        assert_eq!(actual, complete(&model, [1, 0], [512, 55]));
        assert_eq!(*observations.borrow(), vec![recipe(&base).clone()]);
        samples.reverse();
    }
}

#[test]
fn covered_native_function_does_not_adopt_and_later_takeover_preserves_partial_other_channel() {
    let model = model();
    let base = complete(&model, [0, 0], [40, 20]);
    let mut selected = native(&model, 0, 0, 100, 2);
    selected.activation_mix = 0.5;
    let samples = [
        native(&model, 0, 1, u32::MAX, 1),
        selected,
        native(&model, 1, 1, u32::MAX - 7, 3),
    ];
    let observations = RefCell::new(Vec::new());
    let resolver = |value: &AttributeValue, address: &DynamicValueAddress| {
        observations.borrow_mut().push(recipe(value).clone());
        adopt(&model, value, address)
    };
    let actual = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &FamilyCompositionContext {
            resolve_adoption: Some(&resolver),
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(actual, complete(&model, [0, 1], [70, u32::MAX - 7]));
    assert_eq!(
        *observations.borrow(),
        vec![recipe(&complete(&model, [0, 0], [70, 20])).clone()]
    );
}

#[test]
fn native_function_adoption_cannot_reset_another_composed_channel() {
    let model = model();
    let base = complete(&model, [0, 0], [10, 20]);
    let samples = [native(&model, 0, 0, 99, 1), native(&model, 1, 1, 700, 2)];
    let destructive = |value: &AttributeValue, address: &DynamicValueAddress| {
        let adopted = adopt(&model, value, address)?;
        let mut recipe = recipe(&adopted).clone();
        recipe.channels[0].raw = 0;
        let portable = model.predict(&recipe)?;
        Ok(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Direct { recipe, portable },
        )))
    };
    let error = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &FamilyCompositionContext {
            resolve_adoption: Some(&destructive),
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("preserve the other source channels")
    );
    assert_eq!(base, complete(&model, [0, 0], [10, 20]));
}
