//! Nested request-issued graph authority through the captured bridge. This proves
//! retained operand routing and retry ownership; it does not establish physical fitting.
use super::*;

#[test]
fn nested_graph_pending_locator_preserves_original_authority_without_consuming_parent() {
    let sample = sampled(1.5);
    assert_eq!(sample.current_reads.get(), 1);
    with_batch(&sample, |batch, program, _, typed| {
        let frame = Frame::default();
        let destination = FixtureId::new();
        let branch = program.registry().branch();
        let mut parent = batch
            .begin_branch(program, &branch, destination, &no_adoption)
            .unwrap();
        let required_request = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let original_required = parent
            .pending_graph_operation_locator(required_request.request_id)
            .unwrap()
            .unwrap();
        batch
            .resume(
                &mut parent,
                required_request.request_id,
                angles(20., 30.),
                None,
            )
            .unwrap();
        let size_request = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let size = parent
            .pending_graph_operation_locator(size_request.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(size.kind(), PositionGraphOperationKind::Size);

        let mut value = batch
            .begin_graph_branch(
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeValue,
                destination,
                &no_adoption,
            )
            .unwrap();
        let dependency = graph_pending(
            batch
                .advance_graph(&mut value, &frame, &no_adoption)
                .unwrap(),
        );
        let nested = value
            .pending_graph_operation_locator(dependency.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(nested.capture_id(), program.registry().capture_id());
        assert_eq!(nested.kind(), PositionGraphOperationKind::Required);
        assert!(nested.operation_node() == original_required.operation_node());
        assert!(nested.operation_node() != size.operation_node());
        assert!(
            program
                .registry()
                .source_node_for_origin(dependency.origin().unwrap())
                .unwrap()
                .as_ref()
                == Some(nested.operation_node())
        );
        let calls = frame.0.get();
        assert!(
            value
                .pending_graph_operation_locator(Uuid::new_v4())
                .is_err()
        );
        assert!(
            batch
                .resume_graph(
                    &mut value,
                    dependency.request_id,
                    AttributeValue::Normalized(0.4),
                    None,
                )
                .is_err()
        );
        for _ in 0..2 {
            assert_eq!(
                graph_pending(
                    batch
                        .advance_graph(&mut value, &frame, &no_adoption)
                        .unwrap()
                )
                .request_id,
                dependency.request_id
            );
            assert_eq!(
                value
                    .pending_graph_operation_locator(dependency.request_id)
                    .unwrap(),
                Some(nested.clone())
            );
        }
        assert_eq!(frame.0.get(), calls);

        let mut incoming = batch
            .begin_graph_branch(
                program,
                &branch,
                &nested,
                PositionGraphOperationOperand::RequiredIncoming,
                destination,
                &no_adoption,
            )
            .unwrap();
        assert_eq!(
            ready(
                batch
                    .advance_graph(&mut incoming, &frame, &no_adoption)
                    .unwrap()
            ),
            target(9, 30., 40.)
        );
        assert_eq!(frame.0.get(), calls);
        assert!(
            incoming
                .pending_graph_operation_locator(dependency.request_id)
                .is_err()
        );
        batch.recycle_graph(incoming).unwrap();
        assert_eq!(
            value.pending_request().unwrap().request_id,
            dependency.request_id
        );
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            size_request.request_id
        );
        batch
            .resume_graph(&mut value, dependency.request_id, angles(25., 35.), None)
            .unwrap();
        assert_eq!(
            ready(
                batch
                    .advance_graph(&mut value, &frame, &no_adoption)
                    .unwrap()
            ),
            angles(25., 35.)
        );
        assert!(
            value
                .pending_graph_operation_locator(dependency.request_id)
                .is_err()
        );
        batch.recycle_graph(value).unwrap();

        // A real captured-source failure revokes the pending driver's authority even
        // after the source recovers; it cannot issue a nested locator for its old request.
        let mut failed = batch
            .begin_graph_branch(
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeValue,
                destination,
                &no_adoption,
            )
            .unwrap();
        let failed_request = graph_pending(
            batch
                .advance_graph(&mut failed, &frame, &no_adoption)
                .unwrap(),
        );
        assert!(
            failed
                .pending_graph_operation_locator(failed_request.request_id)
                .unwrap()
                .is_some()
        );
        *typed.failure.borrow_mut() =
            Some(IntentError("injected captured source failure".into()).into());
        assert!(
            batch
                .advance_graph(&mut failed, &frame, &no_adoption)
                .is_err()
        );
        *typed.failure.borrow_mut() = None;
        assert!(
            failed
                .pending_graph_operation_locator(failed_request.request_id)
                .is_err()
        );
        batch.recycle_graph(failed).unwrap();
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            size_request.request_id
        );
        assert!(parent.completed_value().is_none());
        assert_eq!(
            sample.current_reads.get(),
            1,
            "nested replay never resamples Current"
        );
        batch.recycle(parent).unwrap();
    });
}
