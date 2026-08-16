use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use std::{cmp::Ordering, fmt, str::FromStr};

/// A lossless Cue address made from canonical, non-negative integer components.
///
/// Components are stored as digit strings so their size is not limited by a machine integer.
/// Leading zeroes are invalid, and trailing zero components remain significant: `2`, `2.0`, and
/// `2.0.0` are three distinct Cue numbers.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CueNumber(Box<[String]>);

impl CueNumber {
    pub fn components(&self) -> impl ExactSizeIterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    pub fn next_whole(&self) -> Self {
        let mut digits = self.0[0].as_bytes().to_vec();
        let mut carry = true;
        for digit in digits.iter_mut().rev() {
            if !carry {
                break;
            }
            if *digit == b'9' {
                *digit = b'0';
            } else {
                *digit += 1;
                carry = false;
            }
        }
        if carry {
            digits.insert(0, b'1');
        }
        Self(vec![String::from_utf8(digits).expect("Cue digits remain UTF-8")].into_boxed_slice())
    }

    pub fn try_from_legacy_f64(value: f64) -> Result<Self, String> {
        if !value.is_finite() || value < 0.0 {
            return Err("legacy Cue number must be finite and non-negative".into());
        }
        let decimal = expand_decimal_exponent(&value.to_string())?;
        let canonical = decimal
            .split('.')
            .map(|component| {
                let component = component.trim_start_matches('0');
                if component.is_empty() { "0" } else { component }
            })
            .collect::<Vec<_>>()
            .join(".");
        canonical.parse()
    }
}

impl FromStr for CueNumber {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err("Cue number must not be empty".into());
        }
        let mut components = Vec::new();
        for component in value.split('.') {
            if component.is_empty() {
                return Err("Cue number components must not be empty".into());
            }
            if !component.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("Cue number components must contain digits only".into());
            }
            if component.len() > 1 && component.starts_with('0') {
                return Err("Cue number components must not contain leading zeroes".into());
            }
            components.push(component.to_owned());
        }
        Ok(Self(components.into_boxed_slice()))
    }
}

impl fmt::Display for CueNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, component) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(".")?;
            }
            formatter.write_str(component)?;
        }
        Ok(())
    }
}

impl Ord for CueNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        for (left, right) in self.0.iter().zip(other.0.iter()) {
            let component_order = left.len().cmp(&right.len()).then_with(|| left.cmp(right));
            if component_order != Ordering::Equal {
                return component_order;
            }
        }
        self.0.len().cmp(&other.0.len())
    }
}

impl PartialOrd for CueNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for CueNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

struct CueNumberVisitor;

impl<'de> Visitor<'de> for CueNumberVisitor {
    type Value = CueNumber;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a canonical dotted Cue path string or a legacy JSON number")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        value.parse().map_err(E::custom)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        value.to_string().parse().map_err(E::custom)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value < 0 {
            return Err(E::custom("legacy Cue number must be non-negative"));
        }
        value.to_string().parse().map_err(E::custom)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        CueNumber::try_from_legacy_f64(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for CueNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(CueNumberVisitor)
    }
}

macro_rules! from_unsigned {
    ($($type:ty),+ $(,)?) => {$(
        impl From<$type> for CueNumber {
            fn from(value: $type) -> Self {
                Self(vec![value.to_string()].into_boxed_slice())
            }
        }
    )+};
}

from_unsigned!(u8, u16, u32, u64, usize);

fn expand_decimal_exponent(value: &str) -> Result<String, String> {
    let Some((mantissa, exponent)) = value.split_once(['e', 'E']) else {
        return Ok(value.to_owned());
    };
    let exponent = exponent
        .parse::<i32>()
        .map_err(|_| "legacy Cue number has an invalid exponent")?;
    let decimal_index = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
    let digits = mantissa.replace('.', "");
    if decimal_index <= 0 {
        return Ok(format!(
            "0.{}{}",
            "0".repeat((-decimal_index) as usize),
            digits
        ));
    }
    let decimal_index = decimal_index as usize;
    if decimal_index >= digits.len() {
        return Ok(format!(
            "{}{}",
            digits,
            "0".repeat(decimal_index - digits.len())
        ));
    }
    Ok(format!(
        "{}.{}",
        &digits[..decimal_index],
        &digits[decimal_index..]
    ))
}

#[cfg(test)]
mod tests {
    use super::CueNumber;

    fn number(value: &str) -> CueNumber {
        value.parse().unwrap()
    }

    #[test]
    fn parses_displays_and_orders_arbitrary_canonical_integer_paths() {
        let mut values = ["3", "2.2", "2.1.1", "2", "2.1.0", "2.0", "2.1"].map(number);
        values.sort();
        assert_eq!(
            values.map(|value| value.to_string()),
            ["2", "2.0", "2.1", "2.1.0", "2.1.1", "2.2", "3"]
        );
        assert!(number("2.2") < number("2.10"));
        assert_eq!(
            number("999999999999999999999999").to_string(),
            "999999999999999999999999"
        );
    }

    #[test]
    fn rejects_noncanonical_or_malformed_paths() {
        for value in ["", ".", "2.", ".2", "2..1", "02", "2.01", "-2", "2.a"] {
            assert!(value.parse::<CueNumber>().is_err(), "{value}");
        }
    }

    #[test]
    fn serializes_as_a_string_and_reads_legacy_json_numbers() {
        assert_eq!(serde_json::to_string(&number("2.0")).unwrap(), r#""2.0""#);
        assert_eq!(
            serde_json::from_str::<CueNumber>("2.05").unwrap(),
            number("2.5")
        );
        assert_eq!(
            serde_json::from_str::<CueNumber>("1e-7").unwrap(),
            number("0.1")
        );
        assert!(serde_json::from_str::<CueNumber>(r#""2.05""#).is_err());
    }
}
