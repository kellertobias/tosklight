import type { ReplacementProgramProjection } from "../../api/generated/light-wire";
import type {
	ColorAdoptionInput,
	ColorAdoptionOutcome,
	ProgrammerValuesHold,
} from "../../api/colorAdoptionWire";
import type { DisplayedSource } from "./displayedSource";
import type {
	DynamicDefinitionProjection,
	DynamicReferenceProjection,
	ProgrammingDynamicSemanticValue,
} from "../../api/types";
import type { ProgrammingComponent } from "../../api/familyEncoderModels";
import type { ProgrammerValueIntentOperation } from "../../api/programmingComponentEditWire";
import type { AttributeValue } from "../../api/types/playback";

export interface ProgrammerValueTiming {
	fade: boolean;
	fadeMillis: number | null;
	delayMillis: number | null;
}

export interface ProgrammerFixtureValue extends ProgrammerValueTiming {
	replacementProjection?: ReplacementProgramProjection;
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
	programmerOrder: number;
}

export interface ProgrammerGroupValue extends ProgrammerValueTiming {
	replacementProjections?: Record<string, ReplacementProgramProjection>;
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
	laneId: string | null;
	component?: ProgrammingComponent | null;
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
			operation: ProgrammerValueIntentOperation<AttributeValue>;
			undoGroup?: string | null;
			timing: ProgrammerValueTiming;
			/** TL-594: the leased displayed source; omitted keeps latest-accepted adoption. */
			displayedSource?: DisplayedSource | null;
			/** TL-554: Direct reference head and explicit semantic starting colour. */
			colorAdoption?: ColorAdoptionInput | null;
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

/**
 * Ends one stopped control gesture (TL-625). `undoGroup` is the gesture's
 * original `apply_intent` Undo group; it never authors values, so it has no
 * optimistic prediction and is never part of `ProgrammerValuesCommand`.
 */
export interface ProgrammerValuesFinishGestureAction {
	action: "finish_gesture";
	attribute: string;
	undoGroup: string;
}

export type ProgrammerValuesRequestAction =
	| ProgrammerValuesCommand
	| ProgrammerValuesFinishGestureAction;

export interface ProgrammerValuesActionRequest {
	requestId: string;
	expectedRevision: number;
	expectedCaptureModeRevision: number;
	action: ProgrammerValuesRequestAction;
}

/** A fresh request ID plus the stopped gesture's original identity. */
export interface FinishProgrammerValuesGestureInput {
	requestId: string;
	attribute: string;
	undoGroup: string;
	/** A completed discrete step: keep the gesture's unsent edits, queue the Finish behind them. */
	keepAdmittedEdits?: boolean;
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
	/** TL-594/TL-554: why the edit was held quietly (no mutation, revision or Undo). */
	hold?: ProgrammerValuesHold;
	/** TL-554: the semantic starting value the first semantic edit of a Direct value adopted. */
	colorAdoption?: ColorAdoptionOutcome;
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
		operation: ProgrammerValueIntentOperation<AttributeValue>;
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
	/**
	 * Locally drops the unsent `apply_intent` rows of one stopped gesture
	 * (its `undoGroup`) and returns how many were dropped. The dispatched row
	 * still settles; no backend action is sent. Optional so view-level fakes
	 * stay valid; the mounted writer always provides it.
	 */
	cancelGesture?(undoGroup: string): number;
	/**
	 * Drops the gesture's unsent edits, then queues exactly one Finish behind
	 * any in-flight edit on the same FIFO. Optional so view-level fakes stay
	 * valid; the mounted writer always provides it.
	 */
	finishGesture?(
		input: FinishProgrammerValuesGestureInput,
	): Promise<ProgrammerValuesActionOutcome | null>;
}
