pub(super) fn strip_prefix_word<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    let remainder = &value[prefix.len()..];
    (remainder.is_empty() || remainder.starts_with(char::is_whitespace))
        .then(|| remainder.trim_start())
}

pub(super) fn starts_with_words(value: &str, words: &[&str]) -> bool {
    value
        .split_whitespace()
        .zip(words)
        .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
        && value.split_whitespace().count() >= words.len()
}

pub(super) fn is_selection_command(value: &str) -> bool {
    let trimmed = value.trim_start();
    let mut characters = trimmed.chars();
    if matches!(characters.next(), Some('F' | 'f' | 'G' | 'g'))
        && characters
            .next()
            .is_some_and(|value| value.is_ascii_digit())
    {
        return true;
    }
    ["FIXTURE", "GROUP", "DEGRP"].iter().any(|prefix| {
        trimmed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            && trimmed
                .get(prefix.len()..)
                .is_some_and(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace))
    })
}

pub(super) fn contains_word(value: &str, word: &str) -> bool {
    value
        .split_whitespace()
        .any(|candidate| candidate.eq_ignore_ascii_case(word))
}

pub(super) fn last_word_is_any(value: &str, expected: &[&str]) -> bool {
    value.split_whitespace().next_back().is_some_and(|word| {
        expected
            .iter()
            .any(|candidate| word.eq_ignore_ascii_case(candidate))
    })
}

pub(super) fn ends_operator(value: &str) -> bool {
    value
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|character| matches!(character, '+' | '-'))
}

pub(super) fn replace_last_word(value: &str, replacement: &str) -> String {
    let trimmed = value.trim_end();
    let start = trimmed
        .char_indices()
        .rev()
        .find_map(|(index, character)| character.is_whitespace().then_some(index + 1))
        .unwrap_or(0);
    format!("{}{replacement}", &trimmed[..start])
}

pub fn remove_command_token(value: &str) -> String {
    let trimmed = value.trim_end();
    let Some(last) = trimmed.chars().next_back() else {
        return String::new();
    };
    if last.is_ascii_digit() || matches!(last, '.' | '-' | '+') {
        let end = trimmed.len() - last.len_utf8();
        return trimmed[..end].trim_end().to_owned();
    }
    let mut start = trimmed.len();
    for (index, character) in trimmed.char_indices().rev() {
        if character.is_ascii_alphabetic() {
            start = index;
        } else {
            break;
        }
    }
    trimmed[..start].trim_end().to_owned()
}

pub(super) fn collapse_whitespace(value: &str) -> String {
    let trailing = value.chars().next_back().is_some_and(char::is_whitespace);
    let mut collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if trailing && !collapsed.is_empty() {
        collapsed.push(' ');
    }
    collapsed
}
