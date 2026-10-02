import type {
	ColorAdoptionInput,
	ColorAdoptionOutcome,
	ProgrammerValuesHold,
} from "../../api/colorAdoptionWire";
import type { DisplayedSource } from "../programmerValues/displayedSource";
import type { ProgrammerValueIntentOperation } from "../../api/programmingComponentEditWire";
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
			operation: ProgrammerValueIntentOperation<AttributeValue>;
			undoGroup?: string | null;
			timing: ProgrammerPreloadValueTiming;
			/** TL-594: the leased Preload displayed source; never a Live lease. */
			displayedSource?: DisplayedSource | null;
			/** TL-554: Direct reference head and explicit semantic starting colour. */
			colorAdoption?: ColorAdoptionInput | null;
	  }
	| {
			action: "apply_indexed_preset";
			expectedSelectionRevision: number;
			attribute: string;
			targets: readonly ProgrammerPreloadIndexedPresetTarget[];
	  }
	| { action: "batch"; mutations: readonly ProgrammerPreloadValuesMutation[] };

/**
 * Ends one stopped Preload control gesture (TL-625). `undoGroup` is the
 * gesture's original `apply_intent` Undo group; it never authors values, so
 * it has no optimistic prediction and is not a `ProgrammerPreloadValuesCommand`.
 */
export interface ProgrammerPreloadValuesFinishGestureAction {
	action: "finish_gesture";
	attribute: string;
	undoGroup: string;
}

export type ProgrammerPreloadValuesRequestAction =
	| ProgrammerPreloadValuesCommand
	| ProgrammerPreloadValuesFinishGestureAction;

export interface ProgrammerPreloadValuesActionRequest {
	requestId: string;
	expectedPreloadRevision: number;
	expectedCaptureModeRevision: number;
	action: ProgrammerPreloadValuesRequestAction;
}

/** A fresh request ID plus the stopped gesture's original identity. */
export interface FinishProgrammerPreloadValuesGestureInput {
	requestId: string;
	attribute: string;
	undoGroup: string;
	/** A completed discrete step: keep the gesture's unsent edits, queue the Finish behind them. */
	keepAdmittedEdits?: boolean;
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
	/** TL-594/TL-554: why the edit was held quietly (no mutation, revision or Undo). */
	hold?: ProgrammerValuesHold;
	/** TL-554: the semantic starting value the first semantic edit of a Direct value adopted. */
	colorAdoption?: ColorAdoptionOutcome;
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
	operation: ProgrammerValueIntentOperation<AttributeValue>;
	undoGroup?: string | null;
	timing: ProgrammerPreloadValueTiming;
	displayedSource?: DisplayedSource | null;
	colorAdoption?: ColorAdoptionInput | null;
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
		input: FinishProgrammerPreloadValuesGestureInput,
	): Promise<ProgrammerPreloadValuesActionOutcome | null>;
}
