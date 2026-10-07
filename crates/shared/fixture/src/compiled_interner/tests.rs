use super::*;

fn profile() -> Arc<FixtureProfile> {
    Arc::new(FixtureProfile::blank())
}

#[test]
fn identical_inputs_share_one_compile_and_any_difference_compiles_again() {
    let mut interner = CompiledModelInterner::<Option<u32>, Arc<u32>>::default();
    let shared = profile();
    let mode = Uuid::from_u128(1);
    let mut compiles = 0;
    let mut get = |interner: &mut CompiledModelInterner<_, _>,
                   profile: &Arc<FixtureProfile>,
                   mode,
                   inputs| {
        interner.get_or_compile(profile, mode, inputs, || {
            compiles += 1;
            Arc::new(compiles)
        })
    };
    let (first, compiled) = get(&mut interner, &shared, mode, Some(7));
    assert!(compiled);
    let (again, compiled) = get(&mut interner, &Arc::clone(&shared), mode, Some(7));
    assert!(!compiled);
    assert!(Arc::ptr_eq(&first, &again));
    // Another calibration, another mode, or another snapshot with equal content each compile.
    assert!(get(&mut interner, &shared, mode, None).1);
    assert!(get(&mut interner, &shared, Uuid::from_u128(2), Some(7)).1);
    let copy = Arc::new(shared.as_ref().clone());
    assert!(get(&mut interner, &copy, mode, Some(7)).1);
    assert_eq!(interner.len(), 4);
    drop(copy);
    interner.retain_live();
    assert_eq!(interner.len(), 3);
}
