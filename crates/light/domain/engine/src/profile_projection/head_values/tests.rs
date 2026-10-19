//! TL-842: a head's channel reads through the patch's compiled addresses, and its colour and
//! level read from its row, answer exactly what the by-name lookup answers, with and without the
//! head's own writes, beside unnumbered values and for values handed over by name.
use super::*;
use crate::contribution::{EngineContribution, EngineContributionResolver};
use chrono::Utc;
use light_core::{MergeMode, TimedValue};
use light_fixture::{ChannelAttribute, PatchedFixture, PatchedHead};

/// A shipped profile patched with one owner per head, the master head keeping the fixture's id.
fn shipped(package: &str) -> PatchedFixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(package);
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = &profile.modes[0];
    let fixture_id = FixtureId::new();
    let mut fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &[]);
    fixture.definition = profile.resolved_definition(mode.id).unwrap();
    fixture.logical_heads = mode
        .heads
        .iter()
        .enumerate()
        .map(|(index, head)| PatchedHead {
            profile_head_id: Some(head.id),
            head_index: index as u16,
            fixture_id: if head.master_shared {
                fixture_id
            } else {
                FixtureId::new()
            },
        })
        .collect();
    fixture
}

fn contribution(fixture: FixtureId, attribute: &str, value: AttributeValue) -> EngineContribution {
    EngineContribution::unscaled(TimedValue {
        fixture_id: fixture,
        attribute: AttributeKey(attribute.into()),
        value,
        priority: 0,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })
}

/// Every numbered read a channel of `owner` makes, against the by-name answer.
fn assert_reads_like_names(view: &HeadValueView<'_, '_>, fixture: &PatchedFixture, owner: usize) {
    let mode = crate::fixture::profile_mode(fixture).unwrap();
    let head = &mode.heads[owner];
    let label = format!("head {owner}");
    for (index, channel) in mode.channels.iter().enumerate() {
        if channel.head_id != head.id {
            continue;
        }
        let mut reads = vec![
            (ChannelAttribute::Canonical, &channel.attribute),
            (ChannelAttribute::Fixture, &channel.fixture_attribute),
        ];
        for (function, compiled) in channel.functions.iter().enumerate() {
            reads.push((ChannelAttribute::Function(function), &compiled.attribute));
        }
        for (which, attribute) in reads {
            assert_eq!(
                view.channel_value(index, which, attribute),
                view.get(attribute),
                "{label} channel {index} {which:?} {}",
                attribute.0
            );
        }
    }
    assert_eq!(
        view.intensity(),
        view.get(AttributeKey::intensity_ref()),
        "{label} intensity"
    );
    assert_eq!(
        view.color(),
        view.get(AttributeKey::color_ref()),
        "{label} color"
    );
}

/// Each head's owner, as the patch numbers it.
fn owners(fixture: &PatchedFixture) -> Vec<FixtureId> {
    let mode = crate::fixture::profile_mode(fixture).unwrap();
    mode.heads
        .iter()
        .enumerate()
        .map(|(index, head)| crate::fixture::profile_head_owner(fixture, index, head))
        .collect()
}

/// Every name a channel of the fixture reads, each with its own level, plus a colour and level
/// for every head and values the patch cannot number.
fn contributions(fixture: &PatchedFixture, stranger: FixtureId) -> Vec<EngineContribution> {
    let mode = crate::fixture::profile_mode(fixture).unwrap();
    let owners = owners(fixture);
    let mut all = Vec::new();
    let mut next = 0.0;
    let mut level = || {
        next = (next + 0.037) % 1.0;
        AttributeValue::Normalized(next)
    };
    for (head, owner) in mode.heads.iter().zip(&owners) {
        all.push(contribution(*owner, "intensity", level()));
        all.push(contribution(
            *owner,
            "color",
            AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1.0, 0.5, 0.0)),
        ));
        for channel in mode
            .channels
            .iter()
            .filter(|channel| channel.head_id == head.id)
        {
            all.push(contribution(*owner, &channel.attribute.0, level()));
            all.push(contribution(*owner, &channel.fixture_attribute.0, level()));
            for function in &channel.functions {
                all.push(contribution(*owner, &function.attribute.0, level()));
            }
        }
    }
    // Unnumbered values send a head through its names; an unpatched owner holds only those.
    all.push(contribution(owners[0], "unnumbered", level()));
    all.push(contribution(stranger, "intensity", level()));
    all.push(contribution(
        stranger,
        "color",
        AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(0.0, 0.5, 1.0)),
    ));
    all
}

#[test]
fn compiled_head_reads_answer_like_named_reads() {
    for package in [
        "showtec--sunstrip-led-rgb-42206.toskfixture",
        "robe--robin-dls-profile.toskfixture",
    ] {
        let fixture = shipped(package);
        let stranger = FixtureId::new();
        let slots =
            std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
        let channels = crate::ChannelSlotIndex::compile(std::slice::from_ref(&fixture), &slots);
        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.extend(contributions(&fixture, stranger));
        let frame = crate::FrameValues::from_frame(resolver.finish().frame.expect("a dense frame"));
        let dense = ProfileValueIndex::new(&frame, &channels);
        let mut by_name = crate::ResolvedValues::default();
        for value in contributions(&fixture, stranger) {
            let key = (value.fixture_id(), value.attribute().clone());
            by_name.insert(key, value.timed_value().value.clone());
        }
        let named = crate::FrameValues::from_maps(by_name, Default::default());
        let scanned = ProfileValueIndex::new(&named, &channels);
        assert!(matches!(scanned, ProfileValueIndex::Scanned { .. }));

        let mode = crate::fixture::profile_mode(&fixture).unwrap();
        let mut distinct = 0;
        for (index, owner) in owners(&fixture).into_iter().enumerate() {
            let own = mode
                .channels
                .iter()
                .filter(|channel| channel.head_id == mode.heads[index].id)
                .collect::<Vec<_>>();
            distinct += own
                .iter()
                .filter(|channel| channel.fixture_attribute != channel.attribute)
                .count();
            for values in [&dense, &scanned] {
                assert_reads_like_names(&HeadValueView::over(values, owner), &fixture, index);
                // The head's own writes and removals win over the frame.
                let mut written = HeadValueView::over(values, owner);
                if let Some(first) = own.first() {
                    written.insert(first.attribute.clone(), AttributeValue::RawDmxExact(7));
                }
                if let Some(last) = own.last() {
                    written.remove(&last.fixture_attribute);
                }
                written.remove(AttributeKey::intensity_ref());
                written.insert(
                    AttributeKey::color_ref().clone(),
                    AttributeValue::Normalized(0.5),
                );
                assert_reads_like_names(&written, &fixture, index);
            }
            let mut local = HeadValueView::local();
            local.insert(AttributeKey::intensity(), AttributeValue::Normalized(0.9));
            assert_reads_like_names(&local, &fixture, index);
        }
        assert!(dense.head_read(owners(&fixture)[0]).overflowed, "{package}");
        // An unpatched owner keeps its level and colour, read only by name.
        let unpatched = HeadValueView::over(&dense, stranger);
        assert!(unpatched.intensity().is_some() && unpatched.color().is_some());
        assert_eq!(
            unpatched.intensity(),
            unpatched.get(AttributeKey::intensity_ref())
        );
        assert_eq!(unpatched.color(), unpatched.get(AttributeKey::color_ref()));
        if package.starts_with("robe") {
            assert!(distinct > 0, "{package} has no manufacturer-named channel");
        }
    }
}
