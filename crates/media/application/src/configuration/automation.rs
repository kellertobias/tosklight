//! Validation of durable playlist and schedule intent before it can replace active settings.

use std::collections::HashSet;

use media_domain::playlist::{AuxiliaryMode, LayerPreset, Playlist};

use super::MediaConfiguration;

/// Validate references and authored values without requiring a mounted media library.
pub fn validate_automation(configuration: &MediaConfiguration) -> Result<(), String> {
    let automation = &configuration.automation;
    let mut playlist_ids = HashSet::new();
    for playlist in &automation.playlists {
        if playlist.id.is_nil() || !playlist_ids.insert(playlist.id) {
            return Err("playlist IDs must be nonempty and unique".into());
        }
        validate_playlist(playlist)?;
    }

    let mut output_ids = HashSet::new();
    for settings in &automation.outputs {
        if !output_ids.insert(settings.output_id) {
            return Err(format!(
                "duplicate automation settings for output {}",
                settings.output_id
            ));
        }
        let output = configuration
            .output(settings.output_id)
            .ok_or_else(|| format!("automation output {} does not exist", settings.output_id))?;
        if !(3..=3600).contains(&settings.dmx_silence_seconds) {
            return Err("dmxSilenceSeconds must be between 3 and 3600 seconds".into());
        }
        if let Some(id) = settings.fallback_playlist_id {
            let playlist = automation
                .playlists
                .iter()
                .find(|playlist| playlist.id == id)
                .ok_or_else(|| format!("fallback playlist {id} does not exist"))?;
            validate_output_layers(playlist, output.personality.layer_count() as usize)?;
        }
    }

    let mut event_ids = HashSet::new();
    for event in &automation.events {
        if event.id.is_nil() || !event_ids.insert(event.id) {
            return Err("event IDs must be nonempty and unique".into());
        }
        if event.name.trim().is_empty() {
            return Err("event name must not be empty".into());
        }
        let output = configuration
            .output(event.output_id)
            .ok_or_else(|| format!("event '{}' selects an unknown output", event.name))?;
        let playlist = automation
            .playlists
            .iter()
            .find(|playlist| playlist.id == event.playlist_id)
            .ok_or_else(|| format!("event '{}' selects an unknown playlist", event.name))?;
        validate_output_layers(playlist, output.personality.layer_count() as usize)?;
        media_domain::schedule::validate_trigger(&event.trigger)
            .map_err(|error| format!("event '{}': {error}", event.name))?;
    }
    Ok(())
}

fn validate_playlist(playlist: &Playlist) -> Result<(), String> {
    if playlist.name.trim().is_empty() {
        return Err("playlist name must not be empty".into());
    }
    if playlist.enabled && playlist.entries.is_empty() {
        return Err(format!(
            "playlist '{}' has no entries; disable it while preparing content",
            playlist.name
        ));
    }
    validate_output_layers(playlist, 8)?;
    for (field, value) in [
        ("masterDimmer", playlist.master_dimmer),
        ("masterVolume", playlist.master_volume),
    ] {
        normalized(field, value)?;
    }
    let mut entries = HashSet::new();
    for entry in &playlist.entries {
        if entry.id.is_nil() || !entries.insert(entry.id) {
            return Err(
                "playlist entry IDs must be nonempty and unique within their playlist".into(),
            );
        }
        if entry.asset_id.as_uuid().is_nil() {
            return Err("playlist assetId must not be empty".into());
        }
        if entry.plays == 0 {
            return Err("playlist entry plays must be at least 1".into());
        }
        if entry.hold_millis == Some(0) {
            return Err("holdMillis must be positive when provided".into());
        }
        if entry
            .hold_millis
            .is_some_and(|hold| entry.fade_millis > hold)
        {
            return Err("fadeMillis must not exceed the entry's display duration".into());
        }
    }
    for auxiliary in &playlist.auxiliary_layers {
        if auxiliary.mode == AuxiliaryMode::Fixed && auxiliary.asset_id.is_none() {
            return Err("fixed auxiliary content needs an assetId".into());
        }
        validate_controls(&auxiliary.controls)?;
    }
    Ok(())
}

fn validate_output_layers(playlist: &Playlist, count: usize) -> Result<(), String> {
    let mut layers = HashSet::new();
    for layer in playlist.layers {
        if layer == 0 || layer > count || !layers.insert(layer) {
            return Err(format!(
                "playlist '{}' needs two distinct playback layers within 1..={count}",
                playlist.name
            ));
        }
    }
    for auxiliary in &playlist.auxiliary_layers {
        if auxiliary.layer == 0 || auxiliary.layer > count || !layers.insert(auxiliary.layer) {
            return Err(format!(
                "playlist '{}' auxiliary layer {} is unavailable, duplicated or used by playback",
                playlist.name, auxiliary.layer
            ));
        }
    }
    Ok(())
}

fn normalized(field: &str, value: f32) -> Result<(), String> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(format!("{field} must be finite and between 0 and 1"));
    }
    Ok(())
}

fn validate_controls(controls: &LayerPreset) -> Result<(), String> {
    for (field, value) in [
        ("dimmer", controls.dimmer),
        ("volume", controls.volume),
        ("grayscale", controls.grayscale),
        ("blur", controls.blur),
        ("tint.red", controls.tint.red),
        ("tint.green", controls.tint.green),
        ("tint.blue", controls.tint.blue),
        ("mask.opacity", controls.mask.opacity),
    ] {
        normalized(field, value)?;
    }
    for value in [
        controls.scale_x,
        controls.scale_y,
        controls.position_x,
        controls.position_y,
        controls.rotation,
        controls.mask.scale_x,
        controls.mask.scale_y,
        controls.mask.position_x,
        controls.mask.position_y,
        controls.model.pan,
        controls.model.tilt,
    ] {
        if !value.is_finite() {
            return Err("auxiliary layer transform values must be finite".into());
        }
    }
    if controls
        .strobe_hz
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return Err("auxiliary strobeHz must be positive and finite".into());
    }
    if !controls.speed_multiplier.factor().is_finite() || controls.speed_multiplier.factor() <= 0.0
    {
        return Err("auxiliary speed multiplier must be positive".into());
    }
    for bank in &controls.effect_banks {
        normalized("effect bank strength", bank.strength)?;
    }
    for effect in &controls.effects {
        normalized("effect mix", effect.mix)?;
        if effect.parameters.iter().any(|value| !value.is_finite()) {
            return Err("effect parameters must be finite".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::playlist::{AutomationOutput, AuxiliaryLayer, PlaylistEntry};
    use media_domain::{AssetId, LayerPersonality};
    use uuid::Uuid;

    fn authored() -> MediaConfiguration {
        let mut configuration = MediaConfiguration::default();
        let playlist = Playlist {
            entries: vec![PlaylistEntry {
                id: Uuid::new_v4(),
                asset_id: AssetId::new(),
                plays: 1,
                hold_millis: Some(5000),
                fade_millis: 1000,
            }],
            ..Playlist::default()
        };
        configuration.automation.outputs.push(AutomationOutput {
            output_id: configuration.outputs[0].id,
            enabled: true,
            fallback_playlist_id: Some(playlist.id),
            dmx_silence_seconds: 10,
        });
        configuration.automation.playlists.push(playlist);
        configuration
    }

    #[test]
    fn existing_configuration_migrates_without_enabling_automation() {
        let configuration = MediaConfiguration::default();
        let mut value: serde_json::Value =
            serde_json::from_str(&super::super::save(&configuration)).unwrap();
        value["version"] = serde_json::json!(10);
        value["configuration"]
            .as_object_mut()
            .unwrap()
            .remove("automation");
        let restored = super::super::load(&value.to_string()).unwrap();
        assert_eq!(restored, configuration);
        assert!(restored.automation.outputs.is_empty());
    }

    #[test]
    fn round_trip_retains_schedule_intent_and_rejects_invalid_references() {
        let mut configuration = authored();
        assert_eq!(
            super::super::load(&super::super::save(&configuration)).unwrap(),
            configuration
        );
        configuration.automation.outputs[0].fallback_playlist_id = Some(Uuid::new_v4());
        assert!(
            validate_automation(&configuration)
                .unwrap_err()
                .contains("does not exist")
        );
    }

    #[test]
    fn compact_outputs_require_an_explicit_valid_pair_and_auxiliary_layers_cannot_collide() {
        let mut configuration = authored();
        configuration.outputs[0].personality = LayerPersonality::TwoLayers;
        assert!(validate_automation(&configuration).is_err());
        configuration.automation.playlists[0].layers = [1, 2];
        validate_automation(&configuration).unwrap();
        configuration.automation.playlists[0]
            .auxiliary_layers
            .push(AuxiliaryLayer {
                layer: 2,
                mode: AuxiliaryMode::Off,
                asset_id: None,
                controls: LayerPreset::default(),
            });
        assert!(validate_automation(&configuration).is_err());
    }

    #[test]
    fn invalid_timing_and_dmx_timeout_are_actionable() {
        let mut configuration = authored();
        configuration.automation.outputs[0].dmx_silence_seconds = 2;
        assert!(
            validate_automation(&configuration)
                .unwrap_err()
                .contains("dmxSilenceSeconds")
        );
        configuration.automation.outputs[0].dmx_silence_seconds = 10;
        configuration.automation.playlists[0].entries[0].fade_millis = 6000;
        assert!(
            validate_automation(&configuration)
                .unwrap_err()
                .contains("fadeMillis")
        );
    }

    #[test]
    fn disabled_empty_playlists_are_editable_drafts() {
        let mut configuration = MediaConfiguration::default();
        configuration.automation.playlists.push(Playlist {
            enabled: false,
            ..Playlist::default()
        });
        validate_automation(&configuration).unwrap();
        configuration.automation.playlists[0].enabled = true;
        assert!(validate_automation(&configuration).is_err());
    }
}
