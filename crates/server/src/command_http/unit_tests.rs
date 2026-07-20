use super::*;

#[test]
fn atomic_family_filter_uses_the_execution_parser_after_timing_clauses() {
    assert_eq!(
        compatibility_only_family("TIME 1 RECORD GROUP 1").unwrap(),
        Some("RECORD")
    );
    assert_eq!(
        compatibility_only_family("DELAY 0.5 CUE 2").unwrap(),
        Some("CUE")
    );
    assert_eq!(
        compatibility_only_family("GROUP 1 AT 50 TIME 1").unwrap(),
        None
    );
}

#[test]
fn atomic_family_filter_owns_only_plain_preset_recording() {
    assert_eq!(compatibility_only_family("RECORD 2.7").unwrap(), None);
    assert_eq!(compatibility_only_family("REC 3 . 9").unwrap(), None);
    assert_eq!(
        compatibility_only_family("RECORD GROUP 7").unwrap(),
        Some("RECORD")
    );
    assert_eq!(
        compatibility_only_family("RECORD CUE 7").unwrap(),
        Some("RECORD")
    );
    assert_eq!(
        compatibility_only_family("RECORD + 2.7").unwrap(),
        Some("RECORD")
    );
    assert_eq!(
        compatibility_only_family("RECORD 2.7 TIME 1").unwrap(),
        Some("RECORD")
    );
}
