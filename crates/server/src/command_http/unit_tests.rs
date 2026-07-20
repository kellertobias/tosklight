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
fn atomic_family_filter_owns_typed_preset_and_group_recording() {
    assert_eq!(compatibility_only_family("RECORD 2.7").unwrap(), None);
    assert_eq!(compatibility_only_family("REC 3 . 9").unwrap(), None);
    assert_eq!(compatibility_only_family("RECORD GROUP 7").unwrap(), None);
    assert_eq!(compatibility_only_family("RECORD + GROUP 7").unwrap(), None);
    assert_eq!(compatibility_only_family("RECORD - GROUP 7").unwrap(), None);
    assert_eq!(compatibility_only_family("DELETE GROUP 7").unwrap(), None);
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

#[test]
fn group_record_parser_owns_only_exact_untimed_group_commands() {
    use light_application::ProgrammingGroupRecordOperation as Operation;

    assert_eq!(
        super::adapter::group_record_command("RECORD GROUP Front").unwrap(),
        Some(("Front".into(), Operation::Overwrite))
    );
    assert_eq!(
        super::adapter::group_record_command("REC + GROUP 07").unwrap(),
        Some(("07".into(), Operation::Merge))
    );
    assert_eq!(
        super::adapter::group_record_command("RECORD - GROUP 07").unwrap(),
        Some(("07".into(), Operation::Subtract))
    );
    assert_eq!(
        super::adapter::group_record_command("DEL GROUP 07").unwrap(),
        Some(("07".into(), Operation::Delete))
    );
    assert_eq!(
        super::adapter::group_record_command("record group Front-Wash").unwrap(),
        Some(("Front-Wash".into(), Operation::Overwrite))
    );
    assert_eq!(
        super::adapter::group_record_command("RECORD GROUP 7 TIME 1").unwrap(),
        None
    );
    assert_eq!(
        super::adapter::group_record_command("DELETE PRESET 7").unwrap(),
        None
    );
}
