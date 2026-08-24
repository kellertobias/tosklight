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

/** The Programmer's pending Preload values. */
export interface ProgrammerPreloadValuesProjection {
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
	| {
			action: "apply_intent";
			fixtureIds: readonly string[];
			groupId?: string | null;
			attribute: string;
			operation:
				| { type: "absolute_set"; value: AttributeValue }
				| { type: "relative_step"; delta: number };
			undoGroup?: string | null;
			timing: ProgrammerPreloadValueTiming;
	  }
	| {
			action: "apply_indexed_preset";
			expectedSelectionRevision: number;
			attribute: string;
			targets: readonly ProgrammerPreloadIndexedPresetTarget[];
	  }
	| { action: "batch"; mutations: readonly ProgrammerPreloadValuesMutation[] };

export interface ProgrammerPreloadValuesActionRequest {
	requestId: string;
	expectedPreloadRevision: number;
	expectedCaptureModeRevision: number;
	action: ProgrammerPreloadValuesCommand;
}

export interface ProgrammerPreloadIndexedPresetTarget {
	fixtureId: string;
	functionId: string;
	expectedProfileRevision: number;
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

export interface ApplyProgrammerPreloadValueIntentInput {
	requestId: string;
	fixtureIds: readonly string[];
	groupId?: string | null;
	attribute: string;
	operation:
		| { type: "absolute_set"; value: AttributeValue }
		| { type: "relative_step"; delta: number };
	undoGroup?: string | null;
	timing: ProgrammerPreloadValueTiming;
}

/** View-owned mutation boundary. It stays dormant until authority is mounted. */
export interface ProgrammerPreloadValuesActions {
	applyIntent(
		input: ApplyProgrammerPreloadValueIntentInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
	applyIndexedPreset(input: {
		requestId: string;
		expectedSelectionRevision: number;
		attribute: string;
		targets: readonly ProgrammerPreloadIndexedPresetTarget[];
	}): Promise<ProgrammerPreloadValuesActionOutcome | null>;
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
