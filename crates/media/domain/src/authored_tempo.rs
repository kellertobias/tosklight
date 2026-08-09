//! The authored-tempo filename token.
//!
//! Import recognises an uppercase `BPM` token in a filename — `My_Loop_BPM119_95.mp4` means
//! 119.95 BPM — and stores the result as typed asset metadata that an operator can correct or
//! remove. Runtime never re-infers it: once the asset exists, this parser is not consulted again,
//! so a rename cannot silently retime a show.

/// Reads the authored tempo out of a filename, if it carries one.
///
/// The token is `BPM`, uppercase, at a word boundary, followed by digits and optionally an
/// underscore and more digits for the fractional part. Unrelated digits elsewhere in the name are
/// never consumed.
pub fn from_filename(filename: &str) -> Option<f64> {
    let bytes = filename.as_bytes();
    let mut search = 0usize;

    while let Some(found) = filename[search..].find("BPM") {
        let start = search + found;
        search = start + 3;

        // A word boundary before the token, so `ABPM120` is not a tempo.
        let preceded_by_word = start
            .checked_sub(1)
            .is_some_and(|before| bytes[before].is_ascii_alphanumeric());
        if preceded_by_word {
            continue;
        }

        if let Some(tempo) = parse_after_token(&filename[search..]) {
            return Some(tempo);
        }
    }
    None
}

fn parse_after_token(rest: &str) -> Option<f64> {
    let whole: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if whole.is_empty() {
        return None;
    }

    let after_whole = &rest[whole.len()..];
    // `_` separates the fractional part: `BPM119_95` is 119.95.
    let fraction: String = after_whole
        .strip_prefix('_')
        .map(|tail| tail.chars().take_while(char::is_ascii_digit).collect())
        .unwrap_or_default();

    let tempo: f64 = if fraction.is_empty() {
        whole.parse().ok()?
    } else {
        format!("{whole}.{fraction}").parse().ok()?
    };

    // Zero is not a tempo, and neither is something implausible enough to be a coincidence.
    (tempo > 0.0 && tempo <= 1_000.0).then_some(tempo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documented_token_parses() {
        assert_eq!(from_filename("My_Loop_BPM119_95.mp4"), Some(119.95));
    }

    #[test]
    fn a_whole_number_tempo_needs_no_fraction() {
        assert_eq!(from_filename("BPM128.mp4"), Some(128.0));
        assert_eq!(from_filename("Drums_BPM90.mov"), Some(90.0));
    }

    #[test]
    fn unrelated_digits_are_not_consumed() {
        assert_eq!(from_filename("Clip2024.mp4"), None);
        assert_eq!(
            from_filename("Shot_2024_BPM90.mp4"),
            Some(90.0),
            "the year is not the tempo"
        );
        assert_eq!(from_filename("128_something.mp4"), None);
        assert_eq!(from_filename("1080p60_clip.mp4"), None);
    }

    #[test]
    fn the_token_is_uppercase_and_at_a_word_boundary() {
        assert_eq!(
            from_filename("my_loop_bpm120.mp4"),
            None,
            "lowercase is not the token"
        );
        assert_eq!(from_filename("ABPM120.mp4"), None, "not a boundary");
        assert_eq!(from_filename("120ABPM.mp4"), None);
        assert_eq!(
            from_filename("Loop-BPM120.mp4"),
            Some(120.0),
            "a dash is a boundary"
        );
        assert_eq!(
            from_filename("Loop BPM120.mp4"),
            Some(120.0),
            "so is a space"
        );
    }

    #[test]
    fn a_token_with_no_digits_is_not_a_tempo() {
        assert_eq!(from_filename("BPM.mp4"), None);
        assert_eq!(from_filename("BPM_.mp4"), None);
        assert_eq!(from_filename("MY_BPM_CLIP.mp4"), None);
    }

    #[test]
    fn a_later_token_is_used_when_an_earlier_one_is_not_a_tempo() {
        assert_eq!(from_filename("BPM_intro_BPM128.mp4"), Some(128.0));
    }

    #[test]
    fn implausible_values_are_refused_rather_than_believed() {
        assert_eq!(from_filename("BPM0.mp4"), None);
        assert_eq!(from_filename("BPM999999.mp4"), None);
        assert_eq!(
            from_filename("BPM1000.mp4"),
            Some(1000.0),
            "the boundary itself is allowed"
        );
    }

    #[test]
    fn a_fraction_of_any_length_parses() {
        assert_eq!(from_filename("BPM119_5.mp4"), Some(119.5));
        assert_eq!(from_filename("BPM119_125.mp4"), Some(119.125));
    }

    #[test]
    fn a_name_with_no_token_has_no_tempo() {
        assert_eq!(from_filename("just-a-clip.mp4"), None);
        assert_eq!(from_filename(""), None);
    }
}
