//! The runtime's programming contract and the TL-560 test-only startup harness.
//!
//! TL-552 raised production to the semantic programming contract
//! (`PROGRAMMING_CONTRACT_VERSION`, 1) and engaged the TL-548 C3 all-family Live path. Before the
//! cutover this module carried an E2E-only opt-in (`e2e-semantic-contract` cargo feature plus
//! `LIGHT_E2E_SEMANTIC_PROGRAMMING_CONTRACT=1`) so the root Playwright semantic specs could run
//! against a contract-1 server. Builds now support the current additive feature contract, so the feature is a retained
//! no-op: `npm run test:e2e-semantic` still builds a separately named binary with it, and that
//! binary behaves exactly like production. No code reads the environment variable any more.
//!
//! The compile-time assertion below fails the build if the production contract or the Live
//! family engagement ever drifts from the cutover.

#[cfg(test)]
use light_core::programming::PROGRAMMING_CONTRACT_VERSION;
use light_core::programming::SUPPORTED_PROGRAMMING_CONTRACT;

/// Production reader capability, including live Preset links and independent Cuelist addresses.
pub(super) const PRODUCTION_PROGRAMMING_CONTRACT: u16 = SUPPORTED_PROGRAMMING_CONTRACT;

// Compile-time proof for every build (production, release, desktop, `e2e-embedded-ui` and the
// no-op `e2e-semantic-contract` build): the runtime reports current feature support and the shared
// all-family Live adapters are engaged.
const _: () = {
    assert!(PRODUCTION_PROGRAMMING_CONTRACT == SUPPORTED_PROGRAMMING_CONTRACT);
    assert!(supported_programming_contract() == SUPPORTED_PROGRAMMING_CONTRACT);
    assert!(live_family_adapters_opted_in());
};

/// The programming contract this runtime supports at startup.
pub(super) const fn supported_programming_contract() -> u16 {
    PRODUCTION_PROGRAMMING_CONTRACT
}

/// TL-560 test-only startup contract harness. `#[cfg(test)]` means it exists only in this crate's
/// own unit-test binary: no library, binary, feature or environment can reach it, and production
/// keeps the `const fn` above. The override is thread-local and scoped by a guard, so parallel
/// tests never observe each other's contract and a panicking test restores the previous value.
/// It changes exactly what `startup_state` reports while it loads (Programmer restore, Engine,
/// Playback and Output runtime gates); every later reader uses `engine.supported_programming_contract()`.
/// Since TL-552 its main use is the contract-0 half of compatibility tests (what an older
/// runtime does with a contract-1 file).
#[cfg(test)]
pub(in crate::runtime) mod startup_contract_override {
    use std::cell::Cell;

    thread_local! {
        static OVERRIDE: Cell<Option<u16>> = const { Cell::new(None) };
    }

    /// Restores the previous override when dropped.
    #[must_use = "the override lasts only while the guard is alive"]
    pub(in crate::runtime) struct StartupContractGuard {
        previous: Option<u16>,
    }

    impl Drop for StartupContractGuard {
        fn drop(&mut self) {
            OVERRIDE.with(|cell| cell.set(self.previous));
        }
    }

    /// Makes `StartupState::load` on this thread run at `contract` while the guard lives.
    pub(in crate::runtime) fn at(contract: u16) -> StartupContractGuard {
        StartupContractGuard {
            previous: OVERRIDE.with(|cell| cell.replace(Some(contract))),
        }
    }

    pub(in crate::runtime) fn current() -> Option<u16> {
        OVERRIDE.with(Cell::get)
    }
}

/// What `startup_state` reports while it loads: the TL-560 test-only override when one is in
/// scope on this thread, otherwise [`supported_programming_contract`].
pub(super) fn startup_programming_contract() -> u16 {
    #[cfg(test)]
    if let Some(contract) = startup_contract_override::current() {
        return contract;
    }
    supported_programming_contract()
}

/// Whether the shared `LiveFamilyAdapters` resource is opted in (TL-548 C3 gate). TL-552: always,
/// so production renders every semantic family through the hybrid Live path. Its `engaged` check
/// still also requires the engine's contract, so a contract-0 engine keeps the legacy path.
pub(super) const fn live_family_adapters_opted_in() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_build_reports_the_semantic_contract_and_engages_the_family_path() {
        assert_eq!(
            PRODUCTION_PROGRAMMING_CONTRACT,
            SUPPORTED_PROGRAMMING_CONTRACT
        );
        assert_eq!(
            supported_programming_contract(),
            SUPPORTED_PROGRAMMING_CONTRACT
        );
        assert_eq!(
            startup_programming_contract(),
            SUPPORTED_PROGRAMMING_CONTRACT
        );
        assert!(live_family_adapters_opted_in());
    }

    #[test]
    fn the_startup_contract_override_is_scoped_thread_local_and_restored() {
        use super::startup_contract_override::{at, current};
        assert_eq!(current(), None);
        {
            let _zero = at(0);
            assert_eq!(current(), Some(0));
            assert_eq!(startup_programming_contract(), 0);
            std::thread::spawn(|| assert_eq!(current(), None, "other threads are unaffected"))
                .join()
                .unwrap();
            {
                let _one = at(PROGRAMMING_CONTRACT_VERSION);
                assert_eq!(current(), Some(PROGRAMMING_CONTRACT_VERSION));
            }
            assert_eq!(current(), Some(0));
        }
        assert_eq!(current(), None);
        let unwound = std::panic::catch_unwind(|| {
            let _zero = at(0);
            panic!("test failure inside the override");
        });
        assert!(unwound.is_err());
        assert_eq!(current(), None, "a panicking test restores the override");
    }
}
