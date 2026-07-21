import type { PatchMutation } from "./contracts";
import type { PatchFixtureCandidate } from "./model";
import { PatchTransportError } from "./transport";

export function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}

export function authorityChanged(): Error {
	return new Error("Patch authority changed before the mutation completed");
}

export function patchMutation(
	requestId: string,
	candidates: readonly PatchFixtureCandidate[],
	removeFixtureIds: readonly string[],
): PatchMutation {
	return {
		requestId,
		fixtures: candidates.map((candidate) => candidate.input),
		removeFixtureIds: [...removeFixtureIds],
	};
}

export function isConflict(error: Error): boolean {
	return (
		error instanceof PatchTransportError &&
		(error.status === 409 || error.currentRevision != null)
	);
}

export function isAmbiguous(error: Error): boolean {
	return (
		!(error instanceof PatchTransportError) ||
		error.retryable ||
		error.status >= 500
	);
}

export function shouldRepair(error: Error): boolean {
	return (
		!(error instanceof PatchTransportError) ||
		isConflict(error) ||
		error.retryable ||
		error.status >= 500
	);
}
