//! Desk-local library resolution and authoritative Internal Audio Player reconciliation.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::{PatchPolicy, PatchedFixture};

use super::timecode_audio_output::{NativeInternalAudioOutput, NativeInternalTransport};

const FOLDER_ATTRIBUTE: &str = "audio.folder";
const FILE_ATTRIBUTE: &str = "audio.file";
const TRANSPORT_ATTRIBUTE: &str = "audio.transport";
const REPEAT_ATTRIBUTE: &str = "audio.repeat";
const VOLUME_ATTRIBUTE: &str = "audio.volume";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct AudioLibraryEntry {
    pub folder: u8,
    pub file: u8,
    pub relative_path: String,
    path: PathBuf,
    kind: AudioFileKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioFileKind {
    Wav,
    Mp3,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct AudioLibraryIndex {
    pub entries: BTreeMap<(u8, u8), AudioLibraryEntry>,
    pub diagnostics: Vec<String>,
}

impl AudioLibraryIndex {
    pub(in crate::runtime) fn scan(root: &Path) -> Self {
        let mut index = Self::default();
        let canonical_root = match root.canonicalize() {
            Ok(root) if root.is_dir() => root,
            Ok(_) => {
                index
                    .diagnostics
                    .push(format!("audio library {} is not a folder", root.display()));
                return index;
            }
            Err(error) => {
                index.diagnostics.push(format!(
                    "audio library {} is unavailable: {error}",
                    root.display()
                ));
                return index;
            }
        };
        let mut folders = addressed_children(&canonical_root, true, &mut index.diagnostics);
        folders.sort_by(|left, right| left.relative_name.cmp(&right.relative_name));
        let mut claimed_folders = HashSet::new();
        for folder in folders {
            if !claimed_folders.insert(folder.address) {
                index.diagnostics.push(format!(
                    "duplicate audio folder {:03}: {} is ignored",
                    folder.address, folder.relative_name
                ));
                continue;
            }
            let mut files = addressed_children(&folder.path, false, &mut index.diagnostics);
            files.sort_by(|left, right| left.relative_name.cmp(&right.relative_name));
            for file in files {
                let Some(kind) = supported_kind(&file.path) else {
                    continue;
                };
                let key = (folder.address, file.address);
                let relative_path = format!("{}/{}", folder.relative_name, file.relative_name);
                let entry = AudioLibraryEntry {
                    folder: folder.address,
                    file: file.address,
                    relative_path: relative_path.clone(),
                    path: file.path,
                    kind,
                };
                if let Some(winner) = index.entries.get(&key) {
                    index.diagnostics.push(format!(
                        "duplicate audio file {:03}/{:03}: {} wins; {} is ignored",
                        key.0, key.1, winner.relative_path, relative_path
                    ));
                } else {
                    index.entries.insert(key, entry);
                }
            }
        }
        index
    }

    fn load(&self, folder: u8, file: u8) -> Result<Vec<u8>, String> {
        let entry = self.entries.get(&(folder, file)).ok_or_else(|| {
            format!("audio source {folder:03}/{file:03} is not present in the selected library")
        })?;
        let bytes = fs::read(&entry.path).map_err(|error| {
            format!("audio file {} cannot be read: {error}", entry.relative_path)
        })?;
        match entry.kind {
            AudioFileKind::Wav => Ok(bytes),
            AudioFileKind::Mp3 => light_application::timeline::normalize_mp3_to_wav(&bytes)
                .map_err(|error| format!("audio file {}: {}", entry.relative_path, error.message)),
        }
    }
}

#[derive(Debug)]
struct AddressedPath {
    address: u8,
    relative_name: String,
    path: PathBuf,
}

fn addressed_children(
    root: &Path,
    folders: bool,
    diagnostics: &mut Vec<String>,
) -> Vec<AddressedPath> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(format!(
                "audio library folder {} cannot be read: {error}",
                root.display()
            ));
            return Vec::new();
        }
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let address = leading_address(&name)?;
            let path = entry.path();
            let canonical = match path.canonicalize() {
                Ok(path) if path.starts_with(root) => path,
                Ok(_) => {
                    diagnostics.push(format!(
                        "audio library entry {name} escapes its confined root"
                    ));
                    return None;
                }
                Err(error) => {
                    diagnostics.push(format!(
                        "audio library entry {name} is unavailable: {error}"
                    ));
                    return None;
                }
            };
            if canonical.is_dir() != folders {
                return None;
            }
            Some(AddressedPath {
                address,
                relative_name: name,
                path: canonical,
            })
        })
        .collect()
}

fn leading_address(name: &str) -> Option<u8> {
    let bytes = name.as_bytes();
    (bytes.len() >= 3 && bytes[..3].iter().all(u8::is_ascii_digit))
        .then(|| name[..3].parse().ok())
        .flatten()
}

fn supported_kind(path: &Path) -> Option<AudioFileKind> {
    match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "wav" => Some(AudioFileKind::Wav),
        "mp3" => Some(AudioFileKind::Mp3),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlayerState {
    source: (u8, u8),
    transport: u8,
    repeat: bool,
    volume: u8,
    cursor_millis: u32,
    transport_changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Default)]
pub(in crate::runtime) struct InternalAudioRuntime {
    libraries: HashMap<String, AudioLibraryIndex>,
    outputs: HashMap<String, NativeInternalAudioOutput>,
    states: HashMap<FixtureId, PlayerState>,
    diagnostics: HashMap<FixtureId, String>,
}

impl InternalAudioRuntime {
    pub(in crate::runtime) fn new(
        library_roots: &BTreeMap<String, String>,
        outputs: BTreeMap<String, NativeInternalAudioOutput>,
    ) -> Self {
        let libraries = library_roots
            .iter()
            .map(|(name, root)| (name.clone(), AudioLibraryIndex::scan(Path::new(root))))
            .collect();
        Self {
            libraries,
            outputs: outputs.into_iter().collect(),
            ..Self::default()
        }
    }

    pub(in crate::runtime) fn reconcile(
        &mut self,
        fixtures: &[PatchedFixture],
        values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
        changed_at: &HashMap<(FixtureId, AttributeKey), chrono::DateTime<chrono::Utc>>,
    ) {
        let players = fixtures
            .iter()
            .filter(|fixture| fixture.definition.patch_policy() == PatchPolicy::Internal)
            .collect::<Vec<_>>();
        let present = players
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<HashSet<_>>();
        for removed in self
            .states
            .keys()
            .copied()
            .filter(|id| !present.contains(id))
            .collect::<Vec<_>>()
        {
            for output in self.outputs.values() {
                let _ = output.remove(removed);
            }
            self.states.remove(&removed);
            self.diagnostics.remove(&removed);
        }
        for fixture in players {
            self.reconcile_fixture(fixture, values, changed_at);
        }
    }

    pub(in crate::runtime) fn status(&self) -> light_wire::v2::internal_audio::InternalAudioStatus {
        let mut player_ids = self.states.keys().copied().collect::<Vec<_>>();
        player_ids.sort_by_key(|id| id.0);
        light_wire::v2::internal_audio::InternalAudioStatus {
            players: player_ids
                .into_iter()
                .map(|fixture_id| {
                    let diagnostic = self.diagnostics.get(&fixture_id).cloned();
                    light_wire::v2::internal_audio::InternalAudioPlayerStatus {
                        fixture_id: fixture_id.0,
                        available: diagnostic.is_none(),
                        diagnostic,
                    }
                })
                .collect(),
            libraries: self
                .libraries
                .iter()
                .map(|(binding, library)| {
                    light_wire::v2::internal_audio::InternalAudioLibraryStatus {
                        binding: binding.clone(),
                        entries: library.entries.len(),
                        diagnostics: library.diagnostics.clone(),
                    }
                })
                .collect(),
        }
    }

    fn reconcile_fixture(
        &mut self,
        fixture: &PatchedFixture,
        values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
        changed_at: &HashMap<(FixtureId, AttributeKey), chrono::DateTime<chrono::Utc>>,
    ) {
        let control_id = fixture
            .logical_heads
            .first()
            .map_or(fixture.fixture_id, |head| head.fixture_id);
        let state = PlayerState {
            source: (
                raw(values, control_id, FOLDER_ATTRIBUTE),
                raw(values, control_id, FILE_ATTRIBUTE),
            ),
            transport: raw(values, control_id, TRANSPORT_ATTRIBUTE),
            repeat: raw(values, control_id, REPEAT_ATTRIBUTE) >= 128,
            volume: raw(values, control_id, VOLUME_ATTRIBUTE),
            cursor_millis: exact_raw(values, control_id, "audio.cursor_millis"),
            transport_changed_at: changed_at
                .get(&(control_id, AttributeKey(TRANSPORT_ATTRIBUTE.into())))
                .copied(),
        };
        let previous = self.states.get(&fixture.fixture_id).copied();
        if previous == Some(state) {
            return;
        }
        let result = self.apply(fixture, state, previous);
        self.states.insert(fixture.fixture_id, state);
        match result {
            Ok(()) => {
                self.diagnostics.remove(&fixture.fixture_id);
            }
            Err(error) => {
                self.diagnostics.insert(fixture.fixture_id, error);
            }
        }
    }

    fn apply(
        &self,
        fixture: &PatchedFixture,
        state: PlayerState,
        previous: Option<PlayerState>,
    ) -> Result<(), String> {
        let library_name = fixture
            .internal_bindings
            .library
            .as_deref()
            .ok_or_else(|| "Audio Player has no logical library binding".to_owned())?;
        let output_name = fixture
            .internal_bindings
            .output
            .as_deref()
            .ok_or_else(|| "Audio Player has no logical output binding".to_owned())?;
        let output = self.outputs.get(output_name).ok_or_else(|| format!("Audio Player output binding {output_name} is not mapped or unavailable on this desk"))?;
        if previous.is_none_or(|old| old.source != state.source) {
            let library = self.libraries.get(library_name).ok_or_else(|| {
                format!("Audio Player library binding {library_name} is not mapped on this desk")
            })?;
            output.prepare(
                fixture.fixture_id,
                &library.load(state.source.0, state.source.1)?,
            )?;
        }
        if previous.is_none_or(|old| old.repeat != state.repeat) {
            output.repeat(fixture.fixture_id, state.repeat)?;
        }
        if previous.is_none_or(|old| old.volume != state.volume) {
            output.volume(fixture.fixture_id, f32::from(state.volume) / 255.0)?;
        }
        let source_changed = previous.is_some_and(|old| old.source != state.source);
        let cursor_jump = previous.is_none_or(|old| {
            state.cursor_millis < old.cursor_millis
                || state.cursor_millis.saturating_sub(old.cursor_millis) > 250
        });
        if source_changed || cursor_jump {
            output.seek(fixture.fixture_id, state.cursor_millis)?;
        }
        if source_changed
            || previous.is_none_or(|old| {
                old.transport != state.transport
                    || old.transport_changed_at != state.transport_changed_at
            })
        {
            let action = match state.transport {
                0..=63 => NativeInternalTransport::Stop,
                64..=127 => NativeInternalTransport::Pause,
                128..=191 => NativeInternalTransport::Play,
                _ => NativeInternalTransport::RestartPlay,
            };
            output.transport(fixture.fixture_id, action)?;
        }
        Ok(())
    }
}

fn raw(
    values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    fixture_id: FixtureId,
    attribute: &str,
) -> u8 {
    values
        .get(&(fixture_id, AttributeKey(attribute.into())))
        .map(|value| match value {
            AttributeValue::RawDmx(raw) => *raw,
            AttributeValue::RawDmxExact(raw) => (*raw).min(255) as u8,
            AttributeValue::Normalized(value) => (value.clamp(0.0, 1.0) * 255.0).round() as u8,
            _ => 0,
        })
        .unwrap_or(0)
}

fn exact_raw(
    values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    fixture_id: FixtureId,
    attribute: &str,
) -> u32 {
    values
        .get(&(fixture_id, AttributeKey(attribute.into())))
        .map(|value| match value {
            AttributeValue::RawDmx(raw) => u32::from(*raw),
            AttributeValue::RawDmxExact(raw) => *raw,
            AttributeValue::Normalized(value) => {
                (value.clamp(0.0, 1.0) * u32::MAX as f32).round() as u32
            }
            _ => 0,
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestLibrary(PathBuf);

    impl TestLibrary {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("tosklight-internal-audio-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn folder(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for TestLibrary {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn leading_addresses_are_exact_decimal_prefixes() {
        assert_eq!(leading_address("001 Ambience"), Some(1));
        assert_eq!(leading_address("255.wav"), Some(255));
        assert_eq!(leading_address("256.wav"), None);
        assert_eq!(leading_address("01.wav"), None);
    }

    #[test]
    fn library_scan_uses_sorted_supported_duplicate_winners() {
        let library = TestLibrary::new();
        let first_folder = library.folder("001 A");
        let ignored_folder = library.folder("001 B");
        fs::write(first_folder.join("001.wav"), []).unwrap();
        fs::write(first_folder.join("001.mp3"), []).unwrap();
        fs::write(first_folder.join("002.txt"), []).unwrap();
        fs::write(first_folder.join("002.wav"), []).unwrap();
        fs::write(ignored_folder.join("003.wav"), []).unwrap();

        let index = AudioLibraryIndex::scan(&library.0);

        assert_eq!(index.entries[&(1, 1)].relative_path, "001 A/001.mp3");
        assert_eq!(index.entries[&(1, 2)].relative_path, "001 A/002.wav");
        assert!(!index.entries.contains_key(&(1, 3)));
        assert!(
            index
                .diagnostics
                .iter()
                .any(|message| message.contains("duplicate audio folder 001"))
        );
        assert!(
            index.diagnostics.iter().any(|message| {
                message.contains("001 A/001.mp3 wins; 001 A/001.wav is ignored")
            })
        );
    }
}
