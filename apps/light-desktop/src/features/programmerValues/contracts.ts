import type {
	DynamicDefinitionProjection,
	DynamicReferenceProjection,
	ProgrammingDynamicSemanticValue,
} from "../../api/types";
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

export interface ProgrammerDynamicValue {
	fixtureId: string;
	attribute: string;
	value: HydratedProgrammingDynamicSemanticValue;
	programmerOrder: number;
	changedAtMillis: number;
}

type HydratedDynamicOn = Extract<
	ProgrammingDynamicSemanticValue,
	{ type: "dynamic_on" }
> & {
	dynamic: DynamicReferenceProjection & {
		embedded_fallback: DynamicDefinitionProjection;
	};
};

export type HydratedProgrammingDynamicSemanticValue =
	| Exclude<ProgrammingDynamicSemanticValue, { type: "dynamic_on" }>
	| HydratedDynamicOn;

/** The Programmer's normal, recordable values. */
export interface ProgrammerValuesProjection {
	revision: number;
	fixtureValues: readonly ProgrammerFixtureValue[];
	groupValues: readonly ProgrammerGroupValue[];
	dynamicValues?: readonly ProgrammerDynamicValue[];
}

export interface ProgrammerValuesSnapshot {
	cursor: number;
	projection: ProgrammerValuesProjection;
}

export interface ProgrammerFixtureValueAddress {
	fixtureId: string;
	attribute: string;
}

export interface ProgrammerDynamicValueAddress
	extends ProgrammerFixtureValueAddress {
	instanceLink: string | null;
}

export interface ProgrammerGroupValueAddress {
	groupId: string;
	attribute: string;
}

export interface ProgrammerValuesChange {
	revision: number;
	fixtureValues: readonly ProgrammerFixtureValue[];
	removedFixtureValues: readonly ProgrammerFixtureValueAddress[];
	groupValues: readonly ProgrammerGroupValue[];
	removedGroupValues: readonly ProgrammerGroupValueAddress[];
	dynamicValues: readonly ProgrammerDynamicValue[];
	removedDynamicValues: readonly ProgrammerDynamicValueAddress[];
}

export type ProgrammerValuesEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			change: ProgrammerValuesChange;
	  }
	| {
			/** @deprecated Compatibility for already-buffered in-process projections. */
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
}

export type ProgrammerValuesMutation =
	| {
			action: "set_selection";
			fixtureIds: readonly string[];
			attribute: string;
			value: AttributeValue;
			timing: ProgrammerValueTiming;
	  }
	| {
			action: "set_selection_color_range";
			fixtureIds: readonly string[];
			start: { hue: number; saturation: number };
			end: { hue: number; saturation: number };
			hueTravel: number;
			brightness: number;
			timing: ProgrammerValueTiming;
	  }
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
	| {
			action: "apply_intent";
			fixtureIds: readonly string[];
			groupId?: string | null;
			attribute: string;
			operation:
				| { type: "absolute_set"; value: AttributeValue }
				| { type: "relative_step"; delta: number };
			undoGroup?: string | null;
			timing: ProgrammerValueTiming;
	  }
	| {
			action: "apply_indexed_preset";
			expectedSelectionRevision: number;
			attribute: string;
			targets: readonly ProgrammerIndexedPresetTarget[];
	  }
	| ProgrammerValuesMutation
	| { action: "batch"; mutations: readonly ProgrammerValuesMutation[] }
	| { action: "clear" };

export interface ProgrammerValuesActionRequest {
	requestId: string;
	expectedRevision: number;
	expectedCaptureModeRevision: number;
	action: ProgrammerValuesCommand;
}

export interface ProgrammerIndexedPresetTarget {
	fixtureId: string;
	functionId: string;
	expectedProfileRevision: number;
}

interface ProgrammerValuesOutcomeBase {
	requestId: string;
	correlationId: string;
	revision: number;
	captureModeRevision: number;
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
	applyIntent(input: {
		requestId: string;
		fixtureIds: readonly string[];
		groupId?: string | null;
		attribute: string;
		operation:
			| { type: "absolute_set"; value: AttributeValue }
			| { type: "relative_step"; delta: number };
		undoGroup?: string | null;
		timing: ProgrammerValueTiming;
	}): Promise<ProgrammerValuesActionOutcome | null>;
	applyIndexedPreset(input: {
		requestId: string;
		expectedSelectionRevision: number;
		attribute: string;
		targets: readonly ProgrammerIndexedPresetTarget[];
	}): Promise<ProgrammerValuesActionOutcome | null>;
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
