use super::PortablePatchError;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Computes the same recursively canonical SHA-256 identity used by portable show revisions.
pub fn fixture_profile_content_digest(profile: &Value) -> Result<String, PortablePatchError> {
    let mut canonical = Vec::new();
    write_canonical(profile, &mut canonical)?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical)))
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) -> Result<(), PortablePatchError> {
    match value {
        Value::Array(values) => write_array(values, output),
        Value::Object(values) => write_object(values, output),
        _ => serde_json::to_writer(output, value).map_err(Into::into),
    }
}

fn write_array(values: &[Value], output: &mut Vec<u8>) -> Result<(), PortablePatchError> {
    output.push(b'[');
    for (index, value) in values.iter().enumerate() {
        push_separator(output, index);
        write_canonical(value, output)?;
    }
    output.push(b']');
    Ok(())
}

fn write_object(
    values: &Map<String, Value>,
    output: &mut Vec<u8>,
) -> Result<(), PortablePatchError> {
    let mut entries = values.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
    output.push(b'{');
    for (index, (key, value)) in entries.into_iter().enumerate() {
        push_separator(output, index);
        serde_json::to_writer(&mut *output, key)?;
        output.push(b':');
        write_canonical(value, output)?;
    }
    output.push(b'}');
    Ok(())
}

fn push_separator(output: &mut Vec<u8>, index: usize) {
    if index > 0 {
        output.push(b',');
    }
}
