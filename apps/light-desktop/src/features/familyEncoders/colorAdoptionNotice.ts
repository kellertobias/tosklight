import { useSyncExternalStore } from "react";
import type {
	ColorAdoptionInput,
	ColorAdoptionOutcome,
	ProgrammerValuesHold,
} from "../../api/colorAdoptionWire";

/**
 * TL-554: the explicit-starting-value state of this desk window, shared by every Color surface
 * (encoders, hardware/OSC encoders through the same binding, the Color dialog).
 *
 * - The server holds the first semantic edit of a Direct value whose visible appearance is
 *   unknown (`explicit_color_start_required`): nothing changed and nothing white was invented.
 *   `required` is then set and the surfaces show a quiet notice offering an explicit start.
 * - The operator's choice is sent with the next semantic Color gesture only; a changed outcome
 *   clears it again. `adoption` is the last reported (approximate or explicit) starting value.
 *
 * Runtime-only presentation state: never stored in the show, the Programmer or Undo history.
 */
export interface ColorAdoptionNoticeState {
	required: boolean;
	explicitStart: NonNullable<ColorAdoptionInput["explicitStart"]> | null;
	adoption: ColorAdoptionOutcome | null;
}

const EMPTY: ColorAdoptionNoticeState = {
	required: false,
	explicitStart: null,
	adoption: null,
};
let state = EMPTY;
const listeners = new Set<() => void>();

function publish(next: ColorAdoptionNoticeState) {
	if (next === state) return;
	state = next;
	for (const listener of listeners) listener();
}

export const colorAdoptionNotice = {
	get: () => state,
	subscribe(listener: () => void) {
		listeners.add(listener);
		return () => listeners.delete(listener);
	},
	/** A quiet hold of a Color edit. Only the explicit-start hold raises the notice. */
	held(reason: ProgrammerValuesHold) {
		if (reason === "explicit_color_start_required" && !state.required)
			publish({ ...state, required: true });
	},
	/** Every Color edit outcome: report an adoption once, clear a used explicit start. */
	outcome(outcome: unknown) {
		if (typeof outcome !== "object" || outcome === null) return;
		const { status, colorAdoption, hold } = outcome as {
			status?: string;
			colorAdoption?: ColorAdoptionOutcome;
			hold?: string;
		};
		if (hold) return;
		if (colorAdoption)
			publish({ required: false, explicitStart: null, adoption: colorAdoption });
		else if (status === "changed" && (state.required || state.explicitStart))
			publish({ ...state, required: false, explicitStart: null });
	},
	/** The operator's explicit start, used by the next semantic Color gesture. */
	choose(rgb: readonly [number, number, number]) {
		publish({ ...state, explicitStart: { rgb } });
	},
	dismissAdoption() {
		if (state.adoption) publish({ ...state, adoption: null });
	},
	/** The adoption options a new semantic Color gesture names. */
	semanticInput(): ColorAdoptionInput | null {
		return state.explicitStart ? { explicitStart: state.explicitStart } : null;
	},
	/** Tests: forget everything. */
	reset() {
		publish(EMPTY);
	},
};

export function useColorAdoptionNotice() {
	return useSyncExternalStore(
		colorAdoptionNotice.subscribe,
		colorAdoptionNotice.get,
		colorAdoptionNotice.get,
	);
}
