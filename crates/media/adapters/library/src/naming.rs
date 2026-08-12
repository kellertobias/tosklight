//! How catalog entries are named on disk.
//!
//! This is the only place filenames are parsed or produced. The domain deals in identities and
//! addresses; if a name has to change shape, it changes here and nowhere else.
//!
//! The layout follows the legacy application so an existing library is recognisable:
//! `NNN/III-Optional-Name.ext`, a folder name in `NNN/.info`, thumbnails in `NNN/.thumbs`.

use media_domain::catalog::{FIRST_FILE, LAST_FILE};

/// The extension normalised playback media carries.
pub const CLIP_EXTENSION: &str = "toskclip";

/// The directory a folder's thumbnails live in.
pub const THUMBNAIL_DIRECTORY: &str = ".thumbs";

/// Operator corrections that override metadata embedded during import.
pub const METADATA_DIRECTORY: &str = ".metadata";

/// The file a folder's operator-given name lives in.
pub const FOLDER_NAME_FILE: &str = ".info";

/// A folder's directory name: three digits, zero padded.
pub fn folder_directory(folder: u8) -> String {
    format!("{folder:03}")
}

/// Reads a folder index out of a directory name. Anything that is not exactly three digits is not
/// a library folder, which is what keeps `.thumbs` and `.system` out of the catalog.
pub fn parse_folder_directory(name: &str) -> Option<u8> {
    if name.len() != 3 || !name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    name.parse().ok()
}

/// An item's filename: `III-Name.toskclip`, or `III.toskclip` when it has no name of its own.
pub fn item_filename(file: u8, name: &str) -> String {
    let safe = safe_name(name);
    if safe.is_empty() {
        format!("{file:03}.{CLIP_EXTENSION}")
    } else {
        format!("{file:03}-{safe}.{CLIP_EXTENSION}")
    }
}

/// A thumbnail's filename, matching the legacy layout.
pub fn thumbnail_filename(file: u8) -> String {
    format!("{file:03}-thumb.jpg")
}

pub fn metadata_filename(file: u8) -> String {
    format!("{file:03}.json")
}

/// Reads an item's index and name out of a filename.
///
/// The index has to be exactly three digits at the start, so `2024-Recap.toskclip` is not item 202
/// and a file that does not follow the convention is skipped rather than misread.
pub fn parse_item_filename(filename: &str) -> Option<(u8, String)> {
    let stem = filename.strip_suffix(&format!(".{CLIP_EXTENSION}"))?;
    if stem.len() < 3 {
        return None;
    }
    let (index, rest) = stem.split_at(3);
    if !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let file: u8 = index.parse().ok()?;
    if !(FIRST_FILE..=LAST_FILE).contains(&file) {
        return None;
    }

    let name = match rest.strip_prefix('-') {
        Some(name) if !name.is_empty() => name.to_owned(),
        // `007.toskclip` has no name of its own; the index stands in for one.
        None if rest.is_empty() => index.to_owned(),
        _ => return None,
    };
    Some((file, name))
}

/// Reads an index and a name out of a *source* filename, whatever its extension.
///
/// The same `NNN[-Name]` convention as a library item, but for a file that has not been imported
/// yet: `001-LoopTest.mp4` is file 1 called `LoopTest`, waiting to become `001-LoopTest.toskclip`.
/// The extension is deliberately not checked here — which formats can be imported is the importer's
/// business, not the naming convention's.
pub fn parse_source_filename(filename: &str) -> Option<(u8, String)> {
    let stem = filename.rsplit_once('.').map(|(stem, _)| stem)?;
    if stem.len() < 3 {
        return None;
    }
    let (index, rest) = stem.split_at(3);
    if !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let file: u8 = index.parse().ok()?;
    if !(FIRST_FILE..=LAST_FILE).contains(&file) {
        return None;
    }

    let name = match rest.strip_prefix('-') {
        Some(name) if !name.is_empty() => name.to_owned(),
        None if rest.is_empty() => index.to_owned(),
        _ => return None,
    };
    Some((file, name))
}

/// Strips anything that would make a filename unsafe or ambiguous.
///
/// Path separators, leading dots, and control characters are removed rather than escaped, because
/// a library entry never needs them and a name that can escape its directory is a hazard.
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' => '-',
            control if control.is_control() => ' ',
            other => other,
        })
        .collect();
    cleaned.trim().trim_start_matches('.').trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_directories_are_three_digits() {
        assert_eq!(folder_directory(1), "001");
        assert_eq!(folder_directory(199), "199");
        assert_eq!(parse_folder_directory("001"), Some(1));
        assert_eq!(parse_folder_directory("199"), Some(199));
    }

    #[test]
    fn only_three_digit_directories_are_library_folders() {
        for name in [".thumbs", ".system", "1", "0001", "abc", "01a", ""] {
            assert_eq!(parse_folder_directory(name), None, "{name}");
        }
    }

    #[test]
    fn an_item_filename_round_trips() {
        let filename = item_filename(7, "My Loop");
        assert_eq!(filename, "007-My Loop.toskclip");
        assert_eq!(
            parse_item_filename(&filename),
            Some((7, "My Loop".to_owned()))
        );
    }

    #[test]
    fn an_item_with_no_name_uses_its_index() {
        assert_eq!(item_filename(7, ""), "007.toskclip");
        assert_eq!(
            parse_item_filename("007.toskclip"),
            Some((7, "007".to_owned()))
        );
    }

    #[test]
    fn a_leading_year_is_not_an_index() {
        // `2024-Recap` starts with digits but is not a three-digit index followed by a separator.
        assert_eq!(parse_item_filename("2024-Recap.toskclip"), None);
    }

    #[test]
    fn files_outside_the_usable_range_are_not_items() {
        assert_eq!(parse_item_filename("000-Blank.toskclip"), None);
        assert_eq!(parse_item_filename("255-Blank.toskclip"), None);
        assert_eq!(
            parse_item_filename("001-First.toskclip"),
            Some((1, "First".to_owned()))
        );
        assert_eq!(
            parse_item_filename("254-Last.toskclip"),
            Some((254, "Last".to_owned()))
        );
    }

    #[test]
    fn anything_that_is_not_a_clip_is_skipped() {
        for filename in ["007-Clip.mp4", "007-Clip", "notes.txt", ".DS_Store", ""] {
            assert_eq!(parse_item_filename(filename), None, "{filename}");
        }
    }

    #[test]
    fn a_name_can_never_escape_its_directory() {
        // The properties are what matter, not one hand-computed string: nothing that could
        // traverse a directory survives.
        for hostile in [
            "../../etc/passwd",
            "..\\..\\windows",
            "/absolute/path",
            "a:b",
        ] {
            let safe = safe_name(hostile);
            assert!(!safe.contains('/'), "{hostile} -> {safe}");
            assert!(!safe.contains('\\'), "{hostile} -> {safe}");
            assert!(!safe.contains(':'), "{hostile} -> {safe}");
            assert!(!safe.starts_with('.'), "{hostile} -> {safe}");
        }
        assert!(!safe_name("a\\b").contains('\\'));
        assert!(!safe_name(".hidden").starts_with('.'));
        assert_eq!(safe_name("  spaced  "), "spaced");
    }

    #[test]
    fn a_name_made_entirely_of_unsafe_characters_falls_back_to_the_index() {
        assert_eq!(item_filename(3, "..."), "003.toskclip");
        assert_eq!(item_filename(3, "   "), "003.toskclip");
    }

    #[test]
    fn thumbnails_follow_the_legacy_layout() {
        assert_eq!(thumbnail_filename(7), "007-thumb.jpg");
    }
}
