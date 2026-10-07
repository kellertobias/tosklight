use super::publication_tests::output;
use super::*;

#[test]
fn overlays_preserve_base_controls_revisions_and_compose_after_flash() {
    let output = output();
    let revision = output.apply_runtime_control(Some(0.4), Some(true)).unwrap();
    let fade = output.begin_transition_fade();
    assert!(fade.set_fade_gain(0.5));
    assert_eq!(output.render_options().grand_master, 0.2);
    assert!(output.render_options().blackout);
    assert!(output.set_grand_master_flash(true));
    assert_eq!(output.render_options().grand_master, 0.5);
    let nested = output.begin_transition_fade();
    assert!(nested.set_fade_gain(0.5));
    assert_eq!(output.render_options().grand_master, 0.25);
    assert!(!nested.set_fade_gain(f32::NAN));
    assert!(!nested.set_fade_gain(1.1));
    assert_eq!(output.render_options().grand_master, 0.25);
    let base = output.control_projection();
    assert_eq!(base.grand_master, 0.4);
    assert!(base.blackout);
    assert_eq!(base.revision, revision);
    drop(fade);
    assert_eq!(output.render_options().grand_master, 0.5);
    output.set_grand_master_flash(false);
    drop(nested);
    assert_eq!(output.render_options().grand_master, 0.4);
    assert!(output.render_options().blackout);
    assert_eq!(output.control_projection().revision, revision);
}

#[test]
fn nested_hold_and_blackout_owners_clean_up_independently() {
    let output = output();
    let first = output.begin_transition_hold();
    let second = output.begin_transition_hold();
    let blackout = output.begin_transition_blackout();
    assert!(output.control.lock().effective_hold());
    assert!(output.render_options().blackout);
    assert!(!blackout.set_fade_gain(0.5));
    drop(first);
    assert!(output.control.lock().effective_hold());
    drop(blackout);
    assert!(output.control.lock().effective_hold());
    assert!(!output.render_options().blackout);
    drop(second);
    assert!(!output.control.lock().effective_hold());
    assert!(output.control.lock().transitions.is_empty());
}

#[test]
fn destination_restore_respects_per_field_equal_value_writes_and_revision() {
    for touched in [0, 1, 2, 3] {
        let output = output();
        output
            .apply_runtime_control(Some(0.4), Some(false))
            .unwrap();
        let lease = output.begin_transition_fade();
        assert!(lease.set_fade_gain(0.5));
        let mut revision = output.control_projection().revision;
        if touched & 1 != 0 {
            revision = output.apply_runtime_control(Some(0.4), None).unwrap();
        }
        if touched & 2 != 0 {
            revision = output.apply_runtime_control(None, Some(false)).unwrap();
        }
        let saved = PersistedOutputRuntime {
            revision: 50,
            grand_master: 0.8,
            blackout: true,
            ..Default::default()
        };
        lease.restore_destination_control(&saved);
        let base = output.control_projection();
        assert_eq!(base.grand_master, if touched & 1 != 0 { 0.4 } else { 0.8 });
        assert_eq!(base.blackout, touched & 2 == 0);
        assert_eq!(base.revision, if touched == 0 { 50 } else { revision });
        assert_eq!(
            output.render_options().grand_master,
            base.grand_master * 0.5
        );
        drop(lease);
        assert_eq!(output.render_options().grand_master, base.grand_master);
        assert_eq!(output.control_projection().revision, base.revision);
    }
}

#[test]
fn dropping_failed_transition_preserves_writes_and_other_owners() {
    let output = output();
    let other = output.begin_transition_blackout();
    let revision = (|| -> Result<u64, &'static str> {
        let _hold = output.begin_transition_hold();
        let fade = output.begin_transition_fade();
        assert!(fade.set_fade_gain(0.0));
        output
            .apply_runtime_control(Some(0.6), Some(false))
            .unwrap();
        Err("failed before activation")
    })();
    assert!(revision.is_err());
    assert!(!output.control.lock().effective_hold());
    assert!(output.render_options().blackout);
    assert_eq!(output.render_options().grand_master, 0.6);
    assert_eq!(output.control_projection().revision, 1);
    drop(other);
    assert!(!output.render_options().blackout);
}

#[tokio::test]
async fn cancelling_transition_future_removes_only_its_effects_without_holding_control_lock() {
    let output = output();
    let other = output.begin_transition_blackout();
    let task_output = output.clone();
    let (ready, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _hold = task_output.begin_transition_hold();
        let fade = task_output.begin_transition_fade();
        assert!(fade.set_fade_gain(0.0));
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    wait.await.unwrap();
    // This would deadlock if the owned lease carried the control guard across the await.
    let revision = output
        .apply_runtime_control(Some(0.7), Some(false))
        .unwrap();
    assert!(output.control.lock().effective_hold());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!output.control.lock().effective_hold());
    assert_eq!(output.render_options().grand_master, 0.7);
    assert!(output.render_options().blackout);
    assert_eq!(output.control_projection().revision, revision);
    drop(other);
    assert!(!output.render_options().blackout);
}

#[test]
fn destination_merge_and_subsequent_operator_writes_are_not_undone_by_drop() {
    let output = output();
    let fade = output.begin_transition_fade();
    fade.restore_destination_control(&PersistedOutputRuntime {
        revision: 40,
        grand_master: 0.2,
        blackout: true,
        ..Default::default()
    });
    assert!(fade.set_fade_gain(0.5));
    let revision = output
        .apply_runtime_control(Some(0.6), Some(false))
        .unwrap();
    assert_eq!(revision, 41);
    drop(fade);
    let base = output.control_projection();
    assert_eq!(base.grand_master, 0.6);
    assert!(!base.blackout);
    assert_eq!(base.revision, 41);
}

#[test]
fn legacy_hold_and_owned_hold_do_not_clear_each_other() {
    let output = output();
    output.set_transition_hold(true);
    let owned = output.begin_transition_hold();
    output.set_transition_hold(false);
    assert!(output.control.lock().effective_hold());
    output.set_transition_hold(true);
    drop(owned);
    assert!(output.control.lock().effective_hold());
    output.set_transition_hold(false);
    assert!(!output.control.lock().effective_hold());
}
