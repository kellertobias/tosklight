//! Desk-local library resolution and authoritative Internal Audio Player reconciliation.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::{PatchPolicy, PatchedFixture};

use super::timecode_audio_output::{NativeInternalAudioOutput, NativeInternalTransport};

const FOLDER_ATTRIBUTE: &str = "media.folder";
const FILE_ATTRIBUTE: &str = "media.file";
const PLAY_MODE_ATTRIBUTE: &str = "media.play_mode";
const VOLUME_ATTRIBUTE: &str = "volume";
// Shows patched before TL-367 carry the Audio Player's own attribute names in their embedded
// profile snapshot. They keep playing through the same reconciliation.
const LEGACY_FOLDER_ATTRIBUTE: &str = "audio.folder";
const LEGACY_FILE_ATTRIBUTE: &str = "audio.file";
const LEGACY_TRANSPORT_ATTRIBUTE: &str = "audio.transport";
const LEGACY_REPEAT_ATTRIBUTE: &str = "audio.repeat";
const LEGACY_VOLUME_ATTRIBUTE: &str = "audio.volume";

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
    canonical_root: Option<PathBuf>,
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
        index.canonical_root = Some(canonical_root.clone());
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
        let canonical_root = self
            .canonical_root
            .as_ref()
            .ok_or_else(|| "audio library root is unavailable".to_owned())?;
        let canonical_path = entry.path.canonicalize().map_err(|error| {
            format!("audio file {} is unavailable: {error}", entry.relative_path)
        })?;
        if !canonical_path.starts_with(canonical_root) || !canonical_path.is_file() {
            return Err(format!(
                "audio file {} is outside the selected library root",
                entry.relative_path
            ));
        }
        let bytes = fs::read(&canonical_path).map_err(|error| {
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
            let address = leading_address(&name, folders)?;
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

fn leading_address(name: &str, folder: bool) -> Option<u8> {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || !bytes[..3].iter().all(u8::is_ascii_digit) {
        return None;
    }
    if folder {
        if bytes.len() != 3 {
            return None;
        }
    } else if bytes.get(3) != Some(&b'.') {
        return None;
    }
    let address = name[..3].parse::<u8>().ok()?;
    (address != 0).then_some(address)
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

/// Resolved transport intent, however the patched personality expresses it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Transport {
    Stop,
    Pause,
    Play,
    RestartPlay,
}

impl Transport {
    fn label(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Pause => "pause",
            Self::Play | Self::RestartPlay => "play",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlayerState {
    source: (u8, u8),
    transport: Transport,
    repeat: bool,
    volume: u8,
    cursor_millis: u32,
    transport_changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// What the desk currently plays on one Internal Audio Player voice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct PlayerSnapshot {
    pub folder: u8,
    pub file: u8,
    pub volume_percent: u8,
    pub transport: &'static str,
    pub repeat: bool,
    pub source: Option<String>,
    pub diagnostic: Option<String>,
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

    pub(in crate::runtime) fn replace_library_roots(
        &mut self,
        library_roots: &BTreeMap<String, String>,
    ) {
        self.libraries = library_roots
            .iter()
            .map(|(name, root)| (name.clone(), AudioLibraryIndex::scan(Path::new(root))))
            .collect();
        // Force the next output tick to prepare the currently selected source from the new root.
        self.states.clear();
        self.diagnostics.clear();
    }

    pub(in crate::runtime) fn reconcile(
        &mut self,
        fixtures: &[PatchedFixture],
        values: &light_engine::ResolvedValues,
        changed_at: &light_engine::ResolvedChangedAt,
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
            libraries: {
                let mut libraries = self.libraries.iter().collect::<Vec<_>>();
                libraries.sort_by(|(left, _), (right, _)| left.cmp(right));
                libraries
                    .into_iter()
                    .map(|(binding, library)| {
                        light_wire::v2::internal_audio::InternalAudioLibraryStatus {
                            binding: binding.clone(),
                            entries: library.entries.len(),
                            diagnostics: library.diagnostics.clone(),
                        }
                    })
                    .collect()
            },
        }
    }

    /// Live Internal Audio Player state for one patched player, as the Media pane shows it.
    pub(in crate::runtime) fn player(&self, fixture: &PatchedFixture) -> PlayerSnapshot {
        let state = self.states.get(&fixture.fixture_id).copied();
        let library_name = fixture
            .internal_bindings
            .library
            .as_deref()
            .unwrap_or("default");
        let source = state.and_then(|state| {
            self.libraries
                .get(library_name)
                .and_then(|library| library.entries.get(&state.source))
                .map(|entry| entry.relative_path.clone())
        });
        PlayerSnapshot {
            folder: state.map_or(0, |state| state.source.0),
            file: state.map_or(0, |state| state.source.1),
            volume_percent: state.map_or(0, |state| {
                ((u16::from(state.volume) * 100 + 127) / 255) as u8
            }),
            transport: state.map_or("stop", |state| state.transport.label()),
            repeat: state.is_some_and(|state| state.repeat),
            source,
            diagnostic: self.diagnostics.get(&fixture.fixture_id).cloned(),
        }
    }

    fn reconcile_fixture(
        &mut self,
        fixture: &PatchedFixture,
        values: &light_engine::ResolvedValues,
        changed_at: &light_engine::ResolvedChangedAt,
    ) {
        let control_id = fixture
            .logical_heads
            .first()
            .map_or(fixture.fixture_id, |head| head.fixture_id);
        let attributes = declared_attributes(fixture);
        let play_mode = attributes.contains(PLAY_MODE_ATTRIBUTE);
        let transport_attribute = if play_mode {
            PLAY_MODE_ATTRIBUTE
        } else {
            LEGACY_TRANSPORT_ATTRIBUTE
        };
        let transport_raw = raw(values, control_id, transport_attribute);
        let state = PlayerState {
            source: (
                raw_of(
                    values,
                    control_id,
                    &attributes,
                    FOLDER_ATTRIBUTE,
                    LEGACY_FOLDER_ATTRIBUTE,
                ),
                raw_of(
                    values,
                    control_id,
                    &attributes,
                    FILE_ATTRIBUTE,
                    LEGACY_FILE_ATTRIBUTE,
                ),
            ),
            transport: if play_mode {
                play_mode_transport(transport_raw)
            } else {
                legacy_transport(transport_raw)
            },
            repeat: if play_mode {
                play_mode_repeats(transport_raw)
            } else {
                raw(values, control_id, LEGACY_REPEAT_ATTRIBUTE) >= 128
            },
            volume: raw_of(
                values,
                control_id,
                &attributes,
                VOLUME_ATTRIBUTE,
                LEGACY_VOLUME_ATTRIBUTE,
            ),
            cursor_millis: exact_raw(values, control_id, "audio.cursor_millis"),
            transport_changed_at: changed_at
                .get(&(control_id, AttributeKey(transport_attribute.into())))
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
            .unwrap_or("default");
        let output_name = fixture
            .internal_bindings
            .output
            .as_deref()
            .unwrap_or("default");
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
                Transport::Stop => NativeInternalTransport::Stop,
                Transport::Pause => NativeInternalTransport::Pause,
                Transport::Play => NativeInternalTransport::Play,
                Transport::RestartPlay => NativeInternalTransport::RestartPlay,
            };
            output.transport(fixture.fixture_id, action)?;
        }
        Ok(())
    }
}

fn declared_attributes(fixture: &PatchedFixture) -> HashSet<String> {
    fixture
        .definition
        .heads
        .iter()
        .flat_map(|head| head.parameters.iter())
        .map(|parameter| parameter.attribute.0.clone())
        .collect()
}

/// Prefers the canonical Media attribute and falls back to the pre-TL-367 Audio Player name.
fn raw_of(
    values: &light_engine::ResolvedValues,
    fixture_id: FixtureId,
    attributes: &HashSet<String>,
    canonical: &str,
    legacy: &str,
) -> u8 {
    let attribute = if attributes.contains(canonical) {
        canonical
    } else {
        legacy
    };
    raw(values, fixture_id, attribute)
}

/// Play mode carries transport and repeat together, as the Media personality does.
fn play_mode_transport(raw: u8) -> Transport {
    match raw {
        216..=235 => Transport::Stop,
        236..=255 => Transport::Pause,
        _ => Transport::Play,
    }
}

/// Loop, Reverse, and Bounce repeat; every Once mode plays the source through exactly once.
fn play_mode_repeats(raw: u8) -> bool {
    matches!(raw, 0..=59 | 108..=167)
}

fn legacy_transport(raw: u8) -> Transport {
    match raw {
        0..=63 => Transport::Stop,
        64..=127 => Transport::Pause,
        128..=191 => Transport::Play,
        _ => Transport::RestartPlay,
    }
}

fn raw(values: &light_engine::ResolvedValues, fixture_id: FixtureId, attribute: &str) -> u8 {
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

fn exact_raw(values: &light_engine::ResolvedValues, fixture_id: FixtureId, attribute: &str) -> u32 {
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
    fn play_mode_carries_transport_and_repeat_together() {
        assert_eq!(play_mode_transport(0), Transport::Play);
        assert_eq!(play_mode_transport(60), Transport::Play);
        assert_eq!(play_mode_transport(215), Transport::Play);
        assert_eq!(play_mode_transport(216), Transport::Stop);
        assert_eq!(play_mode_transport(235), Transport::Stop);
        assert_eq!(play_mode_transport(236), Transport::Pause);
        assert_eq!(play_mode_transport(255), Transport::Pause);
        for looping in [0, 20, 40, 59, 108, 128, 148, 167] {
            assert!(play_mode_repeats(looping), "{looping} loops");
        }
        for once in [60, 68, 84, 107, 168, 192, 215, 216, 236] {
            assert!(!play_mode_repeats(once), "{once} plays once");
        }
    }

    /// A show patched before TL-367 carries the old profile snapshot, so its stored `audio.*`
    /// attributes must keep driving the voice even though the shipped package now declares the
    /// canonical Media names.
    #[test]
    fn a_pre_tl367_patch_snapshot_still_addresses_the_voice_through_its_audio_attributes() {
        let fixture = FixtureId(uuid::Uuid::new_v4());
        let legacy_attributes: HashSet<String> = [
            LEGACY_FOLDER_ATTRIBUTE,
            LEGACY_FILE_ATTRIBUTE,
            LEGACY_VOLUME_ATTRIBUTE,
            LEGACY_TRANSPORT_ATTRIBUTE,
            LEGACY_REPEAT_ATTRIBUTE,
        ]
        .iter()
        .map(|attribute| (*attribute).to_owned())
        .collect();
        let canonical_attributes: HashSet<String> = [
            FOLDER_ATTRIBUTE,
            FILE_ATTRIBUTE,
            VOLUME_ATTRIBUTE,
            PLAY_MODE_ATTRIBUTE,
        ]
        .iter()
        .map(|attribute| (*attribute).to_owned())
        .collect();

        let mut values = light_engine::ResolvedValues::default();
        for (attribute, raw) in [
            (LEGACY_FOLDER_ATTRIBUTE, 7u8),
            (LEGACY_FILE_ATTRIBUTE, 12),
            (LEGACY_VOLUME_ATTRIBUTE, 200),
            (FOLDER_ATTRIBUTE, 1),
            (FILE_ATTRIBUTE, 2),
            (VOLUME_ATTRIBUTE, 3),
        ] {
            values.insert(
                (fixture, AttributeKey(attribute.into())),
                AttributeValue::RawDmx(raw),
            );
        }

        // The legacy snapshot reads its own names even while canonical values are present.
        assert_eq!(
            raw_of(
                &values,
                fixture,
                &legacy_attributes,
                FOLDER_ATTRIBUTE,
                LEGACY_FOLDER_ATTRIBUTE
            ),
            7
        );
        assert_eq!(
            raw_of(
                &values,
                fixture,
                &legacy_attributes,
                FILE_ATTRIBUTE,
                LEGACY_FILE_ATTRIBUTE
            ),
            12
        );
        assert_eq!(
            raw_of(
                &values,
                fixture,
                &legacy_attributes,
                VOLUME_ATTRIBUTE,
                LEGACY_VOLUME_ATTRIBUTE
            ),
            200
        );
        // A snapshot that declares the canonical names prefers them.
        assert_eq!(
            raw_of(
                &values,
                fixture,
                &canonical_attributes,
                FOLDER_ATTRIBUTE,
                LEGACY_FOLDER_ATTRIBUTE
            ),
            1
        );

        // Which transport table applies follows the same snapshot, so a legacy 128 still plays
        // and a canonical 216 still stops.
        assert!(!legacy_attributes.contains(PLAY_MODE_ATTRIBUTE));
        assert_eq!(legacy_transport(128), Transport::Play);
        assert!(canonical_attributes.contains(PLAY_MODE_ATTRIBUTE));
        assert_eq!(play_mode_transport(216), Transport::Stop);
    }

    #[test]
    fn legacy_transport_ranges_survive_for_shows_patched_before_play_mode() {
        assert_eq!(legacy_transport(0), Transport::Stop);
        assert_eq!(legacy_transport(64), Transport::Pause);
        assert_eq!(legacy_transport(128), Transport::Play);
        assert_eq!(legacy_transport(192), Transport::RestartPlay);
        assert_eq!(Transport::RestartPlay.label(), "play");
    }

    #[test]
    fn leading_addresses_are_exact_decimal_prefixes() {
        assert_eq!(leading_address("001", true), Some(1));
        assert_eq!(leading_address("255.wav", false), Some(255));
        assert_eq!(leading_address("000.wav", false), None);
        assert_eq!(leading_address("256.wav", false), None);
        assert_eq!(leading_address("01.wav", false), None);
        assert_eq!(leading_address("001 ambience.wav", false), None);
        assert_eq!(leading_address("001 Ambience", true), None);
        assert_eq!(leading_address("0012", true), None);
    }

    #[test]
    fn library_scan_uses_sorted_supported_duplicate_winners() {
        let library = TestLibrary::new();
        let first_folder = library.folder("001");
        let ignored_folder = library.folder("001 ignored");
        fs::write(first_folder.join("001.wav"), []).unwrap();
        fs::write(first_folder.join("001.mp3"), []).unwrap();
        fs::write(first_folder.join("002.txt"), []).unwrap();
        fs::write(first_folder.join("002.wav"), []).unwrap();
        fs::write(ignored_folder.join("003.wav"), []).unwrap();

        let index = AudioLibraryIndex::scan(&library.0);

        assert_eq!(index.entries[&(1, 1)].relative_path, "001/001.mp3");
        assert_eq!(index.entries[&(1, 2)].relative_path, "001/002.wav");
        assert!(!index.entries.contains_key(&(1, 3)));
        assert!(
            index
                .diagnostics
                .iter()
                .any(|message| { message.contains("001/001.mp3 wins; 001/001.wav is ignored") })
        );
    }

    #[test]
    fn replacing_library_roots_rescans_immediately_and_status_is_stable() {
        let first = TestLibrary::new();
        fs::write(first.folder("001").join("001.wav"), []).unwrap();
        let second = TestLibrary::new();
        fs::write(second.folder("002").join("002.wav"), []).unwrap();
        let mut runtime = InternalAudioRuntime::new(
            &BTreeMap::from([
                ("z".to_owned(), first.0.display().to_string()),
                ("a".to_owned(), first.0.display().to_string()),
            ]),
            BTreeMap::new(),
        );

        let initial = runtime.status();
        assert_eq!(
            initial
                .libraries
                .iter()
                .map(|library| library.binding.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        runtime.replace_library_roots(&BTreeMap::from([(
            "default".to_owned(),
            second.0.display().to_string(),
        )]));

        let replaced = runtime.status();
        assert_eq!(replaced.libraries.len(), 1);
        assert_eq!(replaced.libraries[0].binding, "default");
        assert_eq!(replaced.libraries[0].entries, 1);
    }

    #[cfg(unix)]
    #[test]
    fn loading_rechecks_confinement_after_a_scanned_file_is_replaced() {
        use std::os::unix::fs::symlink;

        let library = TestLibrary::new();
        let folder = library.folder("001");
        let file = folder.join("001.wav");
        fs::write(&file, []).unwrap();
        let index = AudioLibraryIndex::scan(&library.0);
        fs::remove_file(&file).unwrap();
        let outside = std::env::temp_dir().join(format!(
            "tosklight-internal-audio-outside-{}",
            uuid::Uuid::new_v4()
        ));
        fs::write(&outside, []).unwrap();
        symlink(&outside, &file).unwrap();

        let error = index.load(1, 1).unwrap_err();
        assert!(error.contains("outside the selected library root"));
        fs::remove_file(outside).unwrap();
    }
}
