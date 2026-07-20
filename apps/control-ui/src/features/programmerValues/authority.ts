import type { ProgrammerValuesProjection } from "./contracts";
import { sameProjection } from "./projectionValue";
import { ProgrammerValuesProtocolError } from "./transport";

export interface ProgrammerValuesAuthorityDecision {
	projection: ProgrammerValuesProjection;
	sequence: number | null;
	publish: boolean;
}

export function chooseProgrammerValuesAuthority(
	authoritative: ProgrammerValuesProjection | null,
	currentSequence: number | null,
	incoming: ProgrammerValuesProjection,
	sequence: number,
): ProgrammerValuesAuthorityDecision {
	if (currentSequence !== null && sequence < currentSequence)
		return {
			projection: authoritative ?? incoming,
			sequence: currentSequence,
			publish: false,
		};
	if (currentSequence === sequence && authoritative) {
		if (sameProjection(authoritative, incoming))
			return { projection: authoritative, sequence, publish: false };
		throw new ProgrammerValuesProtocolError(
			`Conflicting Programmer values events at sequence ${sequence}`,
			sequence,
		);
	}
	if (authoritative && incoming.revision < authoritative.revision)
		throw new ProgrammerValuesProtocolError(
			`Programmer values revision moved backwards from ${authoritative.revision} to ${incoming.revision}`,
			sequence,
		);
	return {
		...chooseProgrammerValuesRevision(authoritative, incoming, currentSequence),
		sequence,
		publish: true,
	};
}

export function chooseProgrammerValuesRevision(
	authoritative: ProgrammerValuesProjection | null,
	incoming: ProgrammerValuesProjection,
	eventSequence: number | null,
) {
	if (!authoritative || incoming.revision > authoritative.revision)
		return { projection: incoming };
	if (incoming.revision < authoritative.revision)
		return { projection: authoritative };
	if (sameProjection(authoritative, incoming))
		return { projection: authoritative };
	throw new ProgrammerValuesProtocolError(
		`Conflicting Programmer values projections at revision ${incoming.revision}`,
		eventSequence,
	);
}
