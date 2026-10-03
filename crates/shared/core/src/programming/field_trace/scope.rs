//! TL-639: the canonical field set as a bit set. Trace queries union, intersect and subtract
//! scopes for every animated target and frame; the former sorted `Arc<[field]>` allocated on
//! each operation. Unit fields are one bit each. Only the parameterized fields (individual
//! wheels and native channels) keep a sorted shared list, which is absent for the common sets.
//! Equality, ordering, iteration and serialization are those of the sorted field list.
use super::ProgrammingTraceField as F;
use std::{cmp::Ordering, sync::Arc};

/// Unit fields in their declaration (and therefore `Ord`) order; the index is the bit.
const UNIT: [F; 23] = [
    F::ColorXyz,
    F::ColorRecipeRed,
    F::ColorRecipeGreen,
    F::ColorRecipeBlue,
    F::ColorRecipeAmber,
    F::WhiteBlend,
    F::Temperature,
    F::Duv,
    F::Uv,
    F::RelativeOutput,
    F::Allocation,
    F::ColorWheels,
    F::Pan,
    F::Tilt,
    F::TargetReference,
    F::TargetX,
    F::TargetY,
    F::TargetZ,
    F::Focus,
    F::Zoom,
    F::ZoomConvention,
    F::NativeColorIdentity,
    F::NativePrediction,
];
/// `ColorWheel(_)` sorts directly after `ColorWheels`, `NativeColorChannel(_)` directly after
/// `ZoomConvention`.
const WHEELS_END: u32 = 12;
const CHANNELS_END: u32 = 21;
const COLOR_WHEELS: u32 = 1 << 11;

const fn unit_bit(field: F) -> Option<u32> {
    Some(
        1 << match field {
            F::ColorXyz => 0,
            F::ColorRecipeRed => 1,
            F::ColorRecipeGreen => 2,
            F::ColorRecipeBlue => 3,
            F::ColorRecipeAmber => 4,
            F::WhiteBlend => 5,
            F::Temperature => 6,
            F::Duv => 7,
            F::Uv => 8,
            F::RelativeOutput => 9,
            F::Allocation => 10,
            F::ColorWheels => 11,
            F::Pan => 12,
            F::Tilt => 13,
            F::TargetReference => 14,
            F::TargetX => 15,
            F::TargetY => 16,
            F::TargetZ => 17,
            F::Focus => 18,
            F::Zoom => 19,
            F::ZoomConvention => 20,
            F::NativeColorIdentity => 21,
            F::NativePrediction => 22,
            F::ColorWheel(_) | F::NativeColorChannel(_) => return None,
        },
    )
}

/// Canonical immutable set. The wheel aggregate subsumes individual wheel queries; it does not
/// claim that an empty constraint collection contains a physical wheel.
#[derive(Clone, Default, Eq, Hash, PartialEq)]
pub struct ProgrammingFieldScope {
    bits: u32,
    /// Sorted, deduplicated parameterized fields; `None` when there are none. No
    /// `ColorWheel(_)` while `ColorWheels` is present.
    extra: Option<Arc<[F]>>,
}

impl ProgrammingFieldScope {
    pub fn new(fields: impl IntoIterator<Item = F>) -> Self {
        let mut bits = 0;
        let mut extra = Vec::new();
        for field in fields {
            match unit_bit(field) {
                Some(bit) => bits |= bit,
                None => extra.push(field),
            }
        }
        Self::canonical(bits, extra)
    }

    pub(super) const fn from_bits(fields: &[F]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < fields.len() {
            bits |= match unit_bit(fields[index]) {
                Some(bit) => bit,
                None => panic!("parameterized field in a unit scope"),
            };
            index += 1;
        }
        Self { bits, extra: None }
    }

    fn canonical(bits: u32, mut extra: Vec<F>) -> Self {
        if bits & COLOR_WHEELS != 0 {
            extra.retain(|field| !matches!(field, F::ColorWheel(_)));
        }
        if extra.is_empty() {
            return Self { bits, extra: None };
        }
        extra.sort_unstable();
        extra.dedup();
        Self {
            bits,
            extra: Some(extra.into()),
        }
    }

    fn extra(&self) -> &[F] {
        self.extra.as_deref().unwrap_or(&[])
    }

    /// The parameterized fields (individual wheels and native channels), sorted.
    pub(super) fn parameterized(&self) -> &[F] {
        self.extra()
    }

    /// Whether every field belongs to `owner`. Parameterized fields are all Color fields.
    pub(super) fn owned_by(&self, owner: crate::programming::ProgrammingOwner) -> bool {
        const fn mask(owner: crate::programming::ProgrammingOwner) -> u32 {
            let mut bits = 0;
            let mut bit = 0;
            while bit < UNIT.len() {
                if UNIT[bit].owner() as u8 == owner as u8 {
                    bits |= 1 << bit;
                }
                bit += 1;
            }
            bits
        }
        const MASKS: [u32; 4] = [
            mask(crate::programming::ProgrammingOwner::Color),
            mask(crate::programming::ProgrammingOwner::Position),
            mask(crate::programming::ProgrammingOwner::Focus),
            mask(crate::programming::ProgrammingOwner::Zoom),
        ];
        let owned = MASKS[match owner {
            crate::programming::ProgrammingOwner::Color => 0,
            crate::programming::ProgrammingOwner::Position => 1,
            crate::programming::ProgrammingOwner::Focus => 2,
            crate::programming::ProgrammingOwner::Zoom => 3,
        }];
        self.bits & !owned == 0
            && (self.extra.is_none() || owner == crate::programming::ProgrammingOwner::Color)
    }

    pub fn empty() -> Self {
        Self::default()
    }

    /// The fields in canonical (sorted) order.
    pub fn fields(&self) -> Fields<'_> {
        Fields {
            bits: self.bits,
            next_bit: 0,
            extra: self.extra(),
        }
    }

    pub fn len(&self) -> usize {
        self.bits.count_ones() as usize + self.extra().len()
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0 && self.extra.is_none()
    }

    pub fn contains(&self, field: F) -> bool {
        match unit_bit(field) {
            Some(bit) => self.bits & bit != 0,
            None => {
                self.extra().binary_search(&field).is_ok()
                    || (matches!(field, F::ColorWheel(_)) && self.bits & COLOR_WHEELS != 0)
            }
        }
    }

    pub(super) fn overlaps(&self, field: F) -> bool {
        self.contains(field)
            || (field == F::ColorWheels
                && self
                    .extra()
                    .iter()
                    .any(|field| matches!(field, F::ColorWheel(_))))
    }

    pub fn union(&self, other: &Self) -> Self {
        if self == other || other.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return other.clone();
        }
        let bits = self.bits | other.bits;
        if other.extra.is_none() && (self.extra.is_none() || bits & COLOR_WHEELS == 0) {
            return Self {
                bits,
                extra: self.extra.clone(),
            };
        }
        if self.extra.is_none() && bits & COLOR_WHEELS == 0 {
            return Self {
                bits,
                extra: other.extra.clone(),
            };
        }
        Self::canonical(
            bits,
            self.extra().iter().chain(other.extra()).copied().collect(),
        )
    }

    pub fn intersection(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }
        // Unit membership is exact, so the unit part is the plain intersection.
        let bits = self.bits & other.bits;
        if self.extra.is_none() && other.extra.is_none() {
            return Self { bits, extra: None };
        }
        Self::canonical(
            bits,
            self.extra()
                .iter()
                .chain(other.extra())
                .copied()
                .filter(|field| self.contains(*field) && other.contains(*field))
                .collect(),
        )
    }

    /// A wildcard minus individual wheels cannot be represented by this finite positive set.
    /// Callers must keep that query unknown, or request individual wheel fields instead.
    pub fn difference(&self, other: &Self) -> Result<Self, crate::programming::IntentError> {
        super::require(
            !(self.bits & COLOR_WHEELS != 0
                && other.bits & COLOR_WHEELS == 0
                && other
                    .extra()
                    .iter()
                    .any(|field| matches!(field, F::ColorWheel(_)))),
            "wheel aggregate minus individual wheels is not an exact field scope",
        )?;
        let bits = self.bits & !other.bits;
        if self.extra.is_none() {
            return Ok(Self { bits, extra: None });
        }
        Ok(Self::canonical(
            bits,
            self.extra()
                .iter()
                .copied()
                .filter(|field| !other.contains(*field))
                .collect(),
        ))
    }
}

/// Canonical-order iteration over one scope.
#[derive(Clone)]
pub struct Fields<'a> {
    bits: u32,
    next_bit: u32,
    extra: &'a [F],
}

impl Fields<'_> {
    fn next_unit_before(&mut self, end: u32) -> Option<F> {
        while self.next_bit < end {
            let bit = self.next_bit;
            self.next_bit += 1;
            if self.bits & (1 << bit) != 0 {
                return Some(UNIT[bit as usize]);
            }
        }
        None
    }

    fn next_extra(&mut self, wheel: bool) -> Option<F> {
        let (&first, rest) = self.extra.split_first()?;
        if matches!(first, F::ColorWheel(_)) == wheel {
            self.extra = rest;
            Some(first)
        } else {
            None
        }
    }
}

impl Iterator for Fields<'_> {
    type Item = F;

    fn next(&mut self) -> Option<F> {
        if self.next_bit < WHEELS_END
            && let Some(field) = self.next_unit_before(WHEELS_END)
        {
            return Some(field);
        }
        if self.next_bit == WHEELS_END
            && let Some(field) = self.next_extra(true)
        {
            return Some(field);
        }
        if self.next_bit < CHANNELS_END
            && let Some(field) = self.next_unit_before(CHANNELS_END)
        {
            return Some(field);
        }
        if self.next_bit == CHANNELS_END
            && let Some(field) = self.next_extra(false)
        {
            return Some(field);
        }
        self.next_unit_before(UNIT.len() as u32)
    }
}

impl PartialOrd for ProgrammingFieldScope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Lexicographic over the sorted fields, exactly as the former slice representation.
impl Ord for ProgrammingFieldScope {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fields().cmp(other.fields())
    }
}

impl std::fmt::Debug for ProgrammingFieldScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ProgrammingFieldScope")
            .field(&self.fields().collect::<Vec<_>>())
            .finish()
    }
}

impl serde::Serialize for ProgrammingFieldScope {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.fields())
    }
}

#[cfg(test)]
mod tests;
