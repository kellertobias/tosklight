use super::*;

fn cached_response(request_id: String) -> CommandOperationResponse {
    CommandOperationResponse {
        request_id,
        outcome: CommandOperationOutcome::Accepted {
            action: CommandAcceptedAction::NoChange,
            applied: None,
            warning: None,
        },
        command_line: command_line_from_state(CommandLineState::default()),
    }
}

#[test]
fn retry_cache_is_globally_bounded_and_rejects_conflicting_reuse() {
    let mut cache = RequestCache::default();
    let session_id = SessionId::new();
    let first_desk_id = Uuid::new_v4();
    let first_request_id = "request-0".to_owned();
    let mut first_fingerprint = [0; 32];
    first_fingerprint[..8].copy_from_slice(&0_u64.to_le_bytes());

    for index in 0..=REQUEST_CACHE_LIMIT {
        let desk_id = if index == 0 {
            first_desk_id
        } else {
            Uuid::new_v4()
        };
        let request_id = format!("request-{index}");
        let mut fingerprint = [0; 32];
        fingerprint[..8].copy_from_slice(&(index as u64).to_le_bytes());
        cache.insert(
            desk_id,
            session_id,
            request_id.clone(),
            fingerprint,
            cached_response(request_id),
        );
    }

    assert_eq!(cache.entries.len(), REQUEST_CACHE_LIMIT);
    assert!(
        cache
            .get(
                first_desk_id,
                session_id,
                &first_request_id,
                &first_fingerprint
            )
            .unwrap()
            .is_none()
    );

    let retained_desk_id = Uuid::new_v4();
    let retained_request_id = "retained".to_owned();
    let retained_fingerprint = [7; 32];
    cache.insert(
        retained_desk_id,
        session_id,
        retained_request_id.clone(),
        retained_fingerprint,
        cached_response(retained_request_id.clone()),
    );
    assert!(
        cache
            .get(retained_desk_id, session_id, &retained_request_id, &[8; 32])
            .is_err()
    );
}

#[test]
fn operation_locks_prune_only_inactive_desks() {
    let state = CommandHttpState::default();
    let first_desk_id = Uuid::new_v4();
    let second_desk_id = Uuid::new_v4();
    let third_desk_id = Uuid::new_v4();
    let first = state.operation_lock(first_desk_id);
    let second = state.operation_lock(second_desk_id);
    assert_eq!(state.operation_locks.lock().len(), 2);

    drop(first);
    let _third = state.operation_lock(third_desk_id);
    let locks = state.operation_locks.lock();
    assert!(!locks.contains_key(&first_desk_id));
    assert!(locks.contains_key(&second_desk_id));
    assert!(locks.contains_key(&third_desk_id));
    drop(second);
}

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
