import type { ProgrammerPreloadValuesProjection } from "./contracts";
import { samePreloadProjection } from "./projectionValue";
import { ProgrammerPreloadValuesProtocolError } from "./transport";

export interface ProgrammerPreloadValuesAuthorityDecision {
	projection: ProgrammerPreloadValuesProjection;
	sequence: number | null;
	publish: boolean;
}

export function choosePreloadAuthority(
	authoritative: ProgrammerPreloadValuesProjection | null,
	currentSequence: number | null,
	incoming: ProgrammerPreloadValuesProjection,
	sequence: number,
): ProgrammerPreloadValuesAuthorityDecision {
	if (currentSequence !== null && sequence < currentSequence)
		return {
			projection: authoritative ?? incoming,
			sequence: currentSequence,
			publish: false,
		};
	if (currentSequence === sequence && authoritative) {
		if (samePreloadProjection(authoritative, incoming))
			return { projection: authoritative, sequence, publish: false };
		throw new ProgrammerPreloadValuesProtocolError(
			`Conflicting Preload Programmer values events at sequence ${sequence}`,
			sequence,
		);
	}
	if (authoritative && incoming.revision < authoritative.revision)
		throw new ProgrammerPreloadValuesProtocolError(
			`Preload Programmer values revision moved backwards from ${authoritative.revision} to ${incoming.revision}`,
			sequence,
		);
	return {
		...choosePreloadRevision(authoritative, incoming, currentSequence),
		sequence,
		publish: true,
	};
}

export function choosePreloadRevision(
	authoritative: ProgrammerPreloadValuesProjection | null,
	incoming: ProgrammerPreloadValuesProjection,
	eventSequence: number | null,
) {
	if (!authoritative || incoming.revision > authoritative.revision)
		return { projection: incoming };
	if (incoming.revision < authoritative.revision)
		return { projection: authoritative };
	if (samePreloadProjection(authoritative, incoming))
		return { projection: authoritative };
	throw new ProgrammerPreloadValuesProtocolError(
		`Conflicting Preload Programmer values projections at revision ${incoming.revision}`,
		eventSequence,
	);
}
