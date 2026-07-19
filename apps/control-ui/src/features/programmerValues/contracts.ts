import type { AttributeValue } from "../../api/types/playback";

export interface ProgrammerValueTiming {
	fade: boolean;
	fadeMillis: number | null;
	delayMillis: number | null;
}

export interface ProgrammerFixtureValue extends ProgrammerValueTiming {
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
	programmerOrder: number;
}

export interface ProgrammerGroupValue extends ProgrammerValueTiming {
	groupId: string;
	attribute: string;
	value: AttributeValue;
	programmerOrder: number;
}

/** Normal, recordable Programmer values owned by one user. */
export interface ProgrammerValuesProjection {
	userId: string;
	revision: number;
	fixtureValues: readonly ProgrammerFixtureValue[];
	groupValues: readonly ProgrammerGroupValue[];
}

export interface ProgrammerValuesSnapshot {
	cursor: number;
	projection: ProgrammerValuesProjection;
}

export type ProgrammerValuesEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			projection: ProgrammerValuesProjection;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export interface ProgrammerValuesScope {
	showId: string;
	userId: string;
}

export type ProgrammerValuesMutation =
	| {
			action: "set_fixture";
			fixtureId: string;
			attribute: string;
			value: AttributeValue;
			timing: ProgrammerValueTiming;
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
			timing: ProgrammerValueTiming;
	  }
	| {
			action: "release_group";
			groupId: string;
			attribute: string;
	  };

export type ProgrammerValuesCommand =
	| ProgrammerValuesMutation
	| { action: "batch"; mutations: readonly ProgrammerValuesMutation[] }
	| { action: "clear" };

export interface ProgrammerValuesActionRequest {
	requestId: string;
	expectedRevision: number;
	action: ProgrammerValuesCommand;
}

interface ProgrammerValuesOutcomeBase {
	requestId: string;
	correlationId: string;
	revision: number;
	replayed: boolean;
	warning: string | null;
}

export type ProgrammerValuesActionOutcome = ProgrammerValuesOutcomeBase &
	(
		| {
				status: "changed";
				projection: ProgrammerValuesProjection;
				eventSequence: number;
		  }
		| {
				status: "no_change";
				projection?: never;
				eventSequence?: never;
		  }
	);

export interface SetProgrammerFixtureValueInput extends ProgrammerValueTiming {
	requestId: string;
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
}

export interface ReleaseProgrammerFixtureValueInput {
	requestId: string;
	fixtureId: string;
	attribute: string;
}

export interface SetProgrammerGroupValueInput extends ProgrammerValueTiming {
	requestId: string;
	groupId: string;
	attribute: string;
	value: AttributeValue;
}

export interface ReleaseProgrammerGroupValueInput {
	requestId: string;
	groupId: string;
	attribute: string;
}

export interface BatchProgrammerValuesInput {
	requestId: string;
	mutations: readonly ProgrammerValuesMutation[];
}

/** View-owned mutation boundary. It stays dormant until authority has been mounted. */
export interface ProgrammerValuesActions {
	setFixtureValue(
		input: SetProgrammerFixtureValueInput,
	): Promise<ProgrammerValuesActionOutcome | null>;
	releaseFixtureValue(
		input: ReleaseProgrammerFixtureValueInput,
	): Promise<ProgrammerValuesActionOutcome | null>;
	setGroupValue(
		input: SetProgrammerGroupValueInput,
	): Promise<ProgrammerValuesActionOutcome | null>;
	releaseGroupValue(
		input: ReleaseProgrammerGroupValueInput,
	): Promise<ProgrammerValuesActionOutcome | null>;
	batch(
		input: BatchProgrammerValuesInput,
	): Promise<ProgrammerValuesActionOutcome | null>;
	clear(requestId: string): Promise<ProgrammerValuesActionOutcome | null>;
}
