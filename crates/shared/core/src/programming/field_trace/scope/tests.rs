//! The bit set must agree with the former sorted-slice scope on every operation.
use super::*;
use uuid::Uuid;

/// The former representation, kept verbatim as the reference.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Reference(Vec<F>);

impl Reference {
    fn new(fields: impl IntoIterator<Item = F>) -> Self {
        let mut fields: Vec<_> = fields.into_iter().collect();
        fields.sort_unstable();
        fields.dedup();
        if fields.contains(&F::ColorWheels) {
            fields.retain(|field| !matches!(field, F::ColorWheel(_)));
        }
        Self(fields)
    }
    fn contains(&self, field: F) -> bool {
        self.0.binary_search(&field).is_ok()
            || (matches!(field, F::ColorWheel(_)) && self.0.binary_search(&F::ColorWheels).is_ok())
    }
    fn overlaps(&self, field: F) -> bool {
        self.contains(field)
            || (field == F::ColorWheels && self.0.iter().any(|f| matches!(f, F::ColorWheel(_))))
    }
    fn union(&self, other: &Self) -> Self {
        Self::new(self.0.iter().chain(other.0.iter()).copied())
    }
    fn intersection(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }
        Self::new(
            self.0
                .iter()
                .chain(other.0.iter())
                .copied()
                .filter(|field| self.contains(*field) && other.contains(*field)),
        )
    }
    fn difference(&self, other: &Self) -> Option<Self> {
        if self.contains(F::ColorWheels)
            && !other.contains(F::ColorWheels)
            && other.0.iter().any(|f| matches!(f, F::ColorWheel(_)))
        {
            return None;
        }
        Some(Self::new(
            self.0
                .iter()
                .copied()
                .filter(|field| !other.contains(*field)),
        ))
    }
}

fn pool() -> Vec<F> {
    let mut fields = UNIT.to_vec();
    fields.extend([F::ColorWheel(0), F::ColorWheel(3), F::ColorWheel(9)]);
    fields.extend([1u128, 7, 42].map(|id| F::NativeColorChannel(Uuid::from_u128(id))));
    fields
}

/// Deterministic xorshift, no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn fields(&mut self, pool: &[F]) -> Vec<F> {
        let count = (self.next() % 7) as usize;
        (0..count)
            .map(|_| pool[(self.next() % pool.len() as u64) as usize])
            .collect()
    }
}

fn same(scope: &ProgrammingFieldScope, reference: &Reference) {
    assert_eq!(scope.fields().collect::<Vec<_>>(), reference.0);
    assert_eq!(scope.len(), reference.0.len());
    assert_eq!(scope.is_empty(), reference.0.is_empty());
    assert_eq!(
        format!("{scope:?}"),
        format!("ProgrammingFieldScope({:?})", reference.0)
    );
    assert_eq!(
        serde_json::to_value(scope).unwrap(),
        serde_json::to_value(&reference.0).unwrap()
    );
}

#[test]
fn bit_scopes_match_the_sorted_slice_reference_on_every_operation() {
    let pool = pool();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let (a, b) = (rng.fields(&pool), rng.fields(&pool));
        let (sa, sb) = (
            ProgrammingFieldScope::new(a.clone()),
            ProgrammingFieldScope::new(b.clone()),
        );
        let (ra, rb) = (Reference::new(a), Reference::new(b));
        same(&sa, &ra);
        same(&sb, &rb);
        assert_eq!(sa == sb, ra == rb);
        assert_eq!(sa.cmp(&sb), ra.cmp(&rb));
        same(&sa.union(&sb), &ra.union(&rb));
        same(&sa.intersection(&sb), &ra.intersection(&rb));
        match (sa.difference(&sb), ra.difference(&rb)) {
            (Ok(scope), Some(reference)) => same(&scope, &reference),
            (Err(_), None) => {}
            (left, right) => panic!("difference disagrees: {left:?} / {right:?}"),
        }
        for owner in [
            crate::programming::ProgrammingOwner::Color,
            crate::programming::ProgrammingOwner::Position,
            crate::programming::ProgrammingOwner::Focus,
            crate::programming::ProgrammingOwner::Zoom,
        ] {
            assert_eq!(
                sa.owned_by(owner),
                ra.0.iter().all(|field| field.owner() == owner),
                "{ra:?} {owner:?}"
            );
        }
        for field in &pool {
            assert_eq!(sa.contains(*field), ra.contains(*field), "{field:?}");
            assert_eq!(sa.overlaps(*field), ra.overlaps(*field), "{field:?}");
        }
    }
}

#[test]
fn unit_order_is_the_declaration_order() {
    let mut sorted = pool();
    sorted.sort_unstable();
    let scope = ProgrammingFieldScope::new(pool());
    let reference = Reference::new(pool());
    assert_eq!(scope.fields().collect::<Vec<_>>(), reference.0);
    assert!(UNIT.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        ProgrammingFieldScope::from_bits(&[F::Pan, F::Tilt]),
        ProgrammingFieldScope::new([F::Tilt, F::Pan])
    );
}
