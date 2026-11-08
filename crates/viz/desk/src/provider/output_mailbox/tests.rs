use super::*;
use crate::provider::output_demand::OutputDemand;
use light_wire::v2::output_control::{OutputDmxSnapshot, OutputFrameIdentity};

fn output(sequence: u64) -> OutputRead {
    OutputRead {
        request: OutputDemand::default().snapshot(),
        output: Box::new(OutputDmxSnapshot {
            frame: Some(OutputFrameIdentity {
                sequence,
                generation: 1,
                sampled_at: "2026-11-08T00:00:00Z".into(),
                tracking: None,
            }),
            ..Default::default()
        }),
        preload: None,
    }
}

fn next_marker(inbox: &Receiver<Message>) -> u64 {
    match inbox.try_recv().unwrap() {
        Message::OutputReady(marker) => marker,
        _ => panic!("expected an output marker"),
    }
}

#[test]
fn stalled_consumer_gets_one_marker_and_the_newest_coherent_output() {
    let (outbox, inbox, mailbox) = ProviderOutbox::channel();
    for sequence in 1..=100 {
        outbox.send_output(output(sequence)).unwrap();
    }
    outbox.send_output(output(99)).unwrap();
    let marker = next_marker(&inbox);
    assert!(inbox.try_recv().is_err(), "no per-frame notifications");
    let latest = mailbox.take(marker).unwrap();
    assert_eq!(latest.output.frame.unwrap().sequence, 100);

    outbox.send_output(output(101)).unwrap();
    let next = next_marker(&inbox);
    assert_ne!(marker, next);
    assert!(
        mailbox.take(marker).is_none(),
        "retired markers cannot take new output"
    );
    assert_eq!(
        mailbox.take(next).unwrap().output.frame.unwrap().sequence,
        101
    );
}

#[test]
fn configuration_barrier_discards_old_output_without_reordering_messages() {
    let (outbox, inbox, mailbox) = ProviderOutbox::channel();
    outbox.send_output(output(100)).unwrap();
    outbox
        .send(Message::Resync("scene changed".into()))
        .unwrap();
    outbox.send_output(output(1)).unwrap();

    assert!(mailbox.take(next_marker(&inbox)).is_none());
    assert!(matches!(inbox.try_recv(), Ok(Message::Resync(reason)) if reason == "scene changed"));
    assert_eq!(
        mailbox
            .take(next_marker(&inbox))
            .unwrap()
            .output
            .frame
            .unwrap()
            .sequence,
        1
    );
}
