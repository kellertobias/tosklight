import type { PatchMutation, PatchPlacement } from "../contracts";
import type { PatchFixtureCandidate } from "./model";
import { PatchTransportError } from "../transport";

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
	placements: readonly PatchPlacement[] = [],
): PatchMutation {
	const placedFixtureIds = new Set(
		placements.flatMap((placement) => placement.fixtureIds),
	);
	return {
		requestId,
		fixtures: candidates.map((candidate) =>
			placedFixtureIds.has(candidate.input.fixtureId)
				? {
						...candidate.input,
						splitPatches: candidate.input.splitPatches.map((assignment) => ({
							...assignment,
							universe: null,
							address: null,
						})),
					}
				: candidate.input,
		),
		removeFixtureIds: [...removeFixtureIds],
		placements: [...placements],
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
