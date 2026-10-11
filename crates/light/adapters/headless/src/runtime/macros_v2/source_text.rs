//! Pure Macro source token and cursor-coordinate helpers.

pub(super) fn source_tokens(raw: &str) -> Vec<(usize, usize, &str)> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, character) in raw.char_indices() {
        if character.is_whitespace() || character == ';' {
            if let Some(token_start) = start.take() {
                tokens.push((token_start, index, &raw[token_start..index]));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(token_start) = start {
        tokens.push((token_start, raw.len(), &raw[token_start..]));
    }
    tokens
}

pub(super) fn source_definitions(source: &str) -> std::collections::BTreeMap<String, String> {
    let mut definitions = std::collections::BTreeMap::new();
    for line in source.lines() {
        for statement in line.split(';') {
            let statement = statement.trim();
            if statement.starts_with('#') || statement.starts_with("//") {
                break;
            }
            let mut parts = statement.trim().splitn(3, char::is_whitespace);
            if !parts
                .next()
                .is_some_and(|keyword| keyword.eq_ignore_ascii_case("DEFINE"))
            {
                continue;
            }
            let identifier = parts.next().unwrap_or_default();
            let expansion = parts.next().unwrap_or_default().trim();
            if valid_define_token(identifier) && !expansion.is_empty() {
                definitions.insert(identifier.to_owned(), expansion.to_owned());
            }
        }
    }
    definitions
}

pub(super) fn starts_with_macro_keyword(command: &str, keyword: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(keyword))
}

pub(super) fn valid_define_token(token: &str) -> bool {
    token.starts_with('_')
        && token.len() > 1
        && token
            .bytes()
            .skip(1)
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub(super) fn utf16_to_byte(value: &str, offset: u32) -> usize {
    let mut utf16 = 0u32;
    for (byte, character) in value.char_indices() {
        if utf16 >= offset {
            return byte;
        }
        utf16 += character.len_utf16() as u32;
        if utf16 > offset {
            return byte;
        }
    }
    value.len()
}

pub(super) fn byte_to_utf16(value: &str, byte: usize) -> u32 {
    value[..byte].encode_utf16().count() as u32
}
