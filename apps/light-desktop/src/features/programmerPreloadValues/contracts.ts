import type { AttributeValue } from "../../api/types/playback";

export interface ProgrammerPreloadValueTiming {
	fade: boolean;
	fadeMillis: number | null;
	delayMillis: number | null;
}

export interface ProgrammerPreloadFixtureValue
	extends ProgrammerPreloadValueTiming {
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
	programmerOrder: number;
}

export interface ProgrammerPreloadGroupValue
	extends ProgrammerPreloadValueTiming {
	groupId: string;
	attribute: string;
	value: AttributeValue;
	programmerOrder: number;
}

/** Pending Preload Programmer values owned by exactly one user. */
export interface ProgrammerPreloadValuesProjection {
	userId: string;
	revision: number;
	fixtureValues: readonly ProgrammerPreloadFixtureValue[];
	groupValues: readonly ProgrammerPreloadGroupValue[];
}

export interface ProgrammerPreloadValuesSnapshot {
	cursor: number;
	projection: ProgrammerPreloadValuesProjection;
}

export type ProgrammerPreloadValuesEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			projection: ProgrammerPreloadValuesProjection;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export interface ProgrammerPreloadValuesScope {
	showId: string;
	userId: string;
}

export type ProgrammerPreloadValuesMutation =
	| {
			action: "set_fixture";
			fixtureId: string;
			attribute: string;
			value: AttributeValue;
			timing: ProgrammerPreloadValueTiming;
	  }
	| {
			action: "release_fixture";
			fixtureId: string;
			attribute: string;
	  }
	| {
			action: "set_group";
			groupId: string;
			attribute: string;
			value: AttributeValue;
			timing: ProgrammerPreloadValueTiming;
	  }
	| {
			action: "release_group";
			groupId: string;
			attribute: string;
	  };

export type ProgrammerPreloadValuesCommand =
	| ProgrammerPreloadValuesMutation
	| { action: "batch"; mutations: readonly ProgrammerPreloadValuesMutation[] };

export interface ProgrammerPreloadValuesActionRequest {
	requestId: string;
	expectedPreloadRevision: number;
	expectedCaptureModeRevision: number;
	action: ProgrammerPreloadValuesCommand;
}

interface ProgrammerPreloadValuesOutcomeBase {
	requestId: string;
	correlationId: string;
	preloadRevision: number;
	captureModeRevision: number;
	replayed: boolean;
	warning: string | null;
}

export type ProgrammerPreloadValuesActionOutcome =
	ProgrammerPreloadValuesOutcomeBase &
		(
			| {
					status: "changed";
					projection: ProgrammerPreloadValuesProjection;
					eventSequence: number;
			  }
			| {
					status: "no_change";
					projection?: never;
					eventSequence?: never;
			  }
		);

export interface SetProgrammerPreloadFixtureValueInput
	extends ProgrammerPreloadValueTiming {
	requestId: string;
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
}

export interface ReleaseProgrammerPreloadFixtureValueInput {
	requestId: string;
	fixtureId: string;
	attribute: string;
}

export interface SetProgrammerPreloadGroupValueInput
	extends ProgrammerPreloadValueTiming {
	requestId: string;
	groupId: string;
	attribute: string;
	value: AttributeValue;
}

export interface ReleaseProgrammerPreloadGroupValueInput {
	requestId: string;
	groupId: string;
	attribute: string;
}

export interface BatchProgrammerPreloadValuesInput {
	requestId: string;
	mutations: readonly ProgrammerPreloadValuesMutation[];
}

/** View-owned mutation boundary. It stays dormant until authority is mounted. */
export interface ProgrammerPreloadValuesActions {
	setFixtureValue(
		input: SetProgrammerPreloadFixtureValueInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
	releaseFixtureValue(
		input: ReleaseProgrammerPreloadFixtureValueInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
	setGroupValue(
		input: SetProgrammerPreloadGroupValueInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
	releaseGroupValue(
		input: ReleaseProgrammerPreloadGroupValueInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
	batch(
		input: BatchProgrammerPreloadValuesInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
}
