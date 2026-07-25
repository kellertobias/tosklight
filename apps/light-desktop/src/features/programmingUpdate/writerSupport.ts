import type {
	ProgrammingUpdateActionOutcome,
	ProgrammingUpdateObjectIdentity,
	ProgrammingUpdateTarget,
} from "./contracts";
import { ProgrammingUpdateTransportError } from "./contracts";

export class StaleProgrammingUpdateAuthority extends Error {}

export function needsExactCue(target: ProgrammingUpdateTarget) {
	return (
		target.type === "cue" &&
		(target.cue_id == null || target.cue_number == null)
	);
}

export function directObject(
	target: ProgrammingUpdateTarget,
): ProgrammingUpdateObjectIdentity | null {
	if (target.type === "cue") return null;
	return { kind: target.type, object_id: target.object_id, object_revision: 0 };
}

export function assertConfirmedProjection(
	outcome: ProgrammingUpdateActionOutcome,
	expected: ProgrammingUpdateObjectIdentity,
) {
	if (
		outcome.projection.kind !== expected.kind ||
		outcome.projection.object_id !== expected.object_id
	)
		throw new Error("Programming Update response changed its storage identity");
}

export function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

export function transportFailure(error: Error) {
	if (!(error instanceof ProgrammingUpdateTransportError)) return null;
	return {
		status: error.status,
		retryable: error.retryable,
		currentShowRevision: error.currentShowRevision,
	};
}
