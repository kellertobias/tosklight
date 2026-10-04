use super::*;
use light_core::FrameAddressResolver;

fn sample(fixture_id: FixtureId, level: f32) -> crate::ContributionSample {
    crate::ContributionSample::independent(TimedValue {
        fixture_id,
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(level),
        priority: 10,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })
}

/// A sample that says where its pair lives is read there, and lands where the name would.
#[test]
fn an_addressed_sample_lands_where_its_name_would() {
    let fixture_id = FixtureId::new();
    let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity", "pan"]);
    let slots = std::sync::Arc::new(crate::SlotTable::compile(3, std::slice::from_ref(&fixture)));
    let addresser = crate::FrameAddresser::new(std::sync::Arc::clone(&slots));
    let address = addresser
        .frame_address(fixture_id, AttributeKey::intensity_ref())
        .expect("a declared pair has an address");
    assert_eq!(address.generation, 3);
    assert!(
        addresser
            .frame_address(fixture_id, &AttributeKey("zoom".into()))
            .is_none(),
        "an undeclared pair has none"
    );

    let mut resolver = EngineContributionResolver::unpooled(&slots);
    resolver.extend_borrowed_samples([&sample(fixture_id, 0.75).at(Some(address))]);
    let values = resolver.finish().named_values();
    assert_eq!(
        values.value(fixture_id, AttributeKey::intensity_ref()),
        Some(&AttributeValue::Normalized(0.75))
    );
}

/// A number from another patch generation is not trusted: the sample is read by name, as a
/// producer that has not caught up with a repatch would need.
#[test]
fn an_address_from_another_generation_falls_back_to_the_name() {
    let fixture_id = FixtureId::new();
    let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["pan", "intensity"]);
    let slots = std::sync::Arc::new(crate::SlotTable::compile(4, std::slice::from_ref(&fixture)));
    let stale = light_core::FrameAddress {
        generation: 3,
        slot: 0,
    };
    let mut resolver = EngineContributionResolver::unpooled(&slots);
    resolver.extend_borrowed_samples([&sample(fixture_id, 0.5).at(Some(stale))]);
    let values = resolver.finish().named_values();
    assert_eq!(
        values.value(fixture_id, AttributeKey::intensity_ref()),
        Some(&AttributeValue::Normalized(0.5))
    );
    assert_eq!(values.value(fixture_id, &AttributeKey("pan".into())), None);
}
