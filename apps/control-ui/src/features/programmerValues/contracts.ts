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

/** Mutation boundary supplied by the eventual API adapter. */
export interface ProgrammerValuesActions {
	setFixtureValue(input: SetProgrammerFixtureValueInput): Promise<boolean>;
	releaseFixtureValue(input: ReleaseProgrammerFixtureValueInput): Promise<boolean>;
	setGroupValue(input: SetProgrammerGroupValueInput): Promise<boolean>;
	releaseGroupValue(input: ReleaseProgrammerGroupValueInput): Promise<boolean>;
	clear(requestId: string): Promise<boolean>;
}
