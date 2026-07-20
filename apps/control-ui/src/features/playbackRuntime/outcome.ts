import type { PlaybackOutcome } from "./contracts";

export function outcomeMatchesScope(
	outcome: PlaybackOutcome,
	showId: string | null,
	deskId: string | null,
) {
	return (
		matchesShow(outcome.projection.scope.show_id, showId) ||
		outcome.related.some((related) =>
			matchesShow(related.projection.scope.show_id, showId),
		) ||
		(outcome.desk != null &&
			matchesDesk(
				outcome.desk.scope.show_id,
				outcome.desk.desk_id,
				showId,
				deskId,
			))
	);
}

export function outcomeShowRevision(
	outcome: PlaybackOutcome,
	showId: string | null,
	current: number | null,
) {
	let revision = current;
	for (const projection of [
		...outcome.related.map((related) => related.projection),
		outcome.projection,
	])
		if (matchesShow(projection.scope.show_id, showId))
			revision = Math.max(revision ?? 0, projection.scope.show_revision);
	return revision;
}

export function outcomeEventSequence(
	outcome: PlaybackOutcome,
	showId: string | null,
	deskId: string | null,
	current: number | null,
) {
	let sequence = current;
	for (const related of outcome.related)
		if (matchesShow(related.projection.scope.show_id, showId))
			sequence = Math.max(sequence ?? 0, related.event_sequence);
	if (
		outcome.event_sequence != null &&
		matchesShow(outcome.projection.scope.show_id, showId)
	)
		sequence = Math.max(sequence ?? 0, outcome.event_sequence);
	if (
		outcome.desk_event_sequence != null &&
		outcome.desk &&
		matchesDesk(outcome.desk.scope.show_id, outcome.desk.desk_id, showId, deskId)
	)
		sequence = Math.max(sequence ?? 0, outcome.desk_event_sequence);
	return sequence;
}

function matchesShow(candidate: string, showId: string | null) {
	return candidate === showId;
}

function matchesDesk(
	candidateShow: string,
	candidateDesk: string,
	showId: string | null,
	deskId: string | null,
) {
	return matchesShow(candidateShow, showId) && candidateDesk === deskId;
}
