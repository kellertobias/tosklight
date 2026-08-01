//! Reading what a fixture package carries: its scan script, its gobo artwork and its model.
//!
//! A patched show hands the visualizer each profile with its assets inlined as data URLs, so
//! everything here is a decode rather than a file read. Anything that cannot be read is named and
//! skipped: a fixture with an unusable asset falls back to what its type implies, never to a hole
//! on the stage.

use viz_scene::Scene;

/// A packaged script arrives as a data URL, exactly as the photograph and the model do.
pub(super) fn decode_script(asset: &str) -> Option<String> {
    let payload = asset.strip_prefix("data:")?;
    let (metadata, encoded) = payload.split_once(',')?;
    let bytes = if metadata.ends_with(";base64") {
        base64_decode(encoded)?
    } else {
        encoded.as_bytes().to_vec()
    };
    String::from_utf8(bytes).ok()
}

/// Identifies one exact script text, so a changed script recompiles and an unchanged one does not.
pub(super) fn script_key(source: &str) -> u64 {
    // FNV-1a. The key only has to separate one revision of a script from the next, and a hash
    // strong enough for that costs nothing next to compiling the script it identifies.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Zero means "no script", so a script that hashes there is nudged off it.
    hash | 1
}

pub(super) fn base64_decode(encoded: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255_u8; 256];
    for (index, byte) in TABLE.iter().enumerate() {
        lookup[*byte as usize] = index as u8;
    }
    let mut out = Vec::with_capacity(encoded.len() / 4 * 3);
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for byte in encoded.bytes() {
        if byte == b'=' || byte.is_ascii_whitespace() {
            continue;
        }
        let value = lookup[byte as usize];
        if value == 255 {
            return None;
        }
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    Some(out)
}

/// The edge, in pixels, every piece of gobo artwork is resampled to.
///
/// A gate covers a few hundred pixels on screen even in a close beam, and every slot on every
/// wheel in the rig shares one GPU array — which has to be one size. This is the compromise:
/// enough for a pattern to read as etched, small enough that a rig of many wheels costs a few
/// megabytes rather than hundreds.
pub const GOBO_ARTWORK_EDGE: u32 = 256;

/// Resolve one profile's declared gobo wheel, decoding artwork into the scene as it goes.
///
/// A profile that declares no wheel gets an empty one, which is the renderer's signal to divide
/// the channel into its own drawn patterns. A slot whose artwork cannot be read keeps its place
/// on the wheel — the slot still exists on the fixture — and says why.
pub(super) fn gobo_wheel(
    profile: &light_fixture::FixtureProfile,
    scene: &mut Scene,
    artwork: &mut std::collections::HashMap<String, Option<u32>>,
    warnings: &mut Vec<String>,
) -> Vec<viz_scene::GoboSlot> {
    let Some(highest) = profile.gobos.iter().map(|gobo| gobo.slot).max() else {
        return Vec::new();
    };
    let mut wheel = vec![viz_scene::GoboSlot::default(); highest as usize + 1];
    for gobo in &profile.gobos {
        let slot = &mut wheel[gobo.slot as usize];
        slot.name = gobo.name.clone().unwrap_or_default();
        let Some(asset) = gobo.artwork_asset.as_ref() else {
            continue;
        };
        let index = match artwork.get(asset) {
            Some(cached) => *cached,
            None => {
                let decoded = match read_gobo_asset(asset) {
                    Ok(image) => {
                        scene.gobo_artwork.push(image);
                        Some(scene.gobo_artwork.len() as u32 - 1)
                    }
                    Err(reason) => {
                        warnings.push(format!(
                            "{} {} gobo slot {}: {reason}",
                            profile.manufacturer, profile.name, gobo.slot
                        ));
                        None
                    }
                };
                artwork.insert(asset.clone(), decoded);
                decoded
            }
        };
        slot.artwork = index;
    }
    wheel
}

/// Read one slot's artwork, which the fixture library stores as a data URL, into a square mask.
pub(super) fn read_gobo_asset(asset: &str) -> Result<viz_scene::GoboArtwork, String> {
    let encoded = asset
        .split_once(";base64,")
        .map(|(_, encoded)| encoded)
        .ok_or_else(|| "the artwork is not an inline data URL".to_owned())?;
    let bytes = decode_base64(encoded).ok_or_else(|| "the artwork is not base64".to_owned())?;
    decode_gobo_artwork(&bytes)
}

/// Read an encoded image into the square mask the renderer samples.
pub fn decode_gobo_artwork(bytes: &[u8]) -> Result<viz_scene::GoboArtwork, String> {
    let image = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    // Glass is a mask: what matters is where light passes, so colour is discarded and the
    // brightest channel decides. Everything is squared to one edge because the whole rig's
    // artwork shares one GPU array.
    let mask = image
        .resize_exact(
            GOBO_ARTWORK_EDGE,
            GOBO_ARTWORK_EDGE,
            image::imageops::FilterType::Lanczos3,
        )
        .to_luma8();
    Ok(viz_scene::GoboArtwork {
        edge: GOBO_ARTWORK_EDGE,
        mask: mask.into_raw(),
    })
}

/// Read a profile's `model_asset`, which the fixture library stores as a data URL.
pub(super) fn read_model_asset(asset: &str) -> Result<viz_scene::FixtureModel, String> {
    let encoded = asset
        .split_once(";base64,")
        .map(|(_, encoded)| encoded)
        .ok_or_else(|| "the model asset is not an inline data URL".to_owned())?;
    let bytes = decode_base64(encoded).ok_or_else(|| "the model asset is not base64".to_owned())?;
    let model = viz_scene::read_glb(&bytes).map_err(|error| error.0)?;
    Ok(model)
}

/// Decode standard base64 with padding.
///
/// The fixture library writes these data URLs itself, so the alphabet is known and a decoder here
/// keeps the projection layer free of another dependency.
pub(super) fn decode_base64(encoded: &str) -> Option<Vec<u8>> {
    let value_of = |character: u8| -> Option<u32> {
        match character {
            b'A'..=b'Z' => Some(u32::from(character - b'A')),
            b'a'..=b'z' => Some(u32::from(character - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(character - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(encoded.len() / 4 * 3);
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for character in encoded.bytes() {
        if character == b'=' || character.is_ascii_whitespace() {
            continue;
        }
        let value = value_of(character)?;
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_the_bytes_the_library_writes() {
        // Standard alphabet with padding, which is what the fixture library emits.
        assert_eq!(decode_base64("TWFu").expect("decodes"), b"Man");
        assert_eq!(decode_base64("TWE=").expect("decodes"), b"Ma");
        assert_eq!(decode_base64("TQ==").expect("decodes"), b"M");
        assert_eq!(
            decode_base64("Z2xURgIAAAA=").expect("decodes"),
            [0x67, 0x6c, 0x54, 0x46, 0x02, 0x00, 0x00, 0x00]
        );
        assert!(decode_base64("not base64!").is_none());
    }

    #[test]
    fn a_model_asset_that_is_not_an_inline_data_url_is_named_not_guessed() {
        let error = read_model_asset("assets/model.glb").expect_err("refused");
        assert!(error.contains("data URL"), "{error}");
    }

    #[test]
    fn a_data_url_that_is_not_a_model_is_refused_by_name() {
        let error = read_model_asset("data:model/gltf-binary;base64,bm90IGEgbW9kZWw=")
            .expect_err("refused");
        assert!(error.contains("GLB"), "{error}");
    }
}
