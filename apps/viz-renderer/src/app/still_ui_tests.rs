use super::*;

#[test]
fn new_invocation_detaches_previous_image_opener_completion() {
    let mut ui = StillUi::default();
    let (sender, receiver) = mpsc::channel::<Result<(), String>>();
    ui.file_action = Some(receiver);
    ui.reset();
    assert!(sender.send(Err("old image opener failed".into())).is_err());
    ui.observe(None);
    assert!(
        ui.error.is_none(),
        "an old opener cannot fail the new frozen invocation"
    );
    assert!(ui.file_action.is_none());
}

#[test]
fn disconnected_opener_is_reported_and_does_not_block_retry() {
    let mut ui = StillUi::default();
    let (sender, receiver) = mpsc::channel::<Result<(), String>>();
    ui.file_action = Some(receiver);
    drop(sender);
    ui.observe(None);
    assert!(ui.file_action.is_none());
    assert!(ui.error.as_ref().unwrap().contains("stopped unexpectedly"));
    assert!(ui.visible());
}
