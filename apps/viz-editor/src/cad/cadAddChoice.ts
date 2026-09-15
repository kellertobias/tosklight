/**
 * Which part each CAD add button places, remembered on this computer.
 *
 * Pressing the button places the part last chosen from its caret menu, or the button's default until
 * one is chosen. A remembered part the button no longer offers falls back to the default, and storage
 * that cannot be read or written only forgets the choice.
 */
import {
	type CadPartKind,
	DEFAULT_PART_PROFILE_IDS,
	type FoundPart,
	findPart,
} from "./venueParts";

const storageKey = (kind: CadPartKind) => `tosklight:viz-editor:cad-add-part:${kind}:v1`;

export function chosenPart(kind: CadPartKind): FoundPart {
	let stored: string | null = null;
	try {
		stored = localStorage.getItem(storageKey(kind));
	} catch {
		stored = null;
	}
	const remembered = stored ? findPart(kind, stored) : undefined;
	return remembered ?? (findPart(kind, DEFAULT_PART_PROFILE_IDS[kind]) as FoundPart);
}

export function rememberPart(kind: CadPartKind, profileId: string) {
	try {
		localStorage.setItem(storageKey(kind), profileId);
	} catch {
		// The part is still placed; only the choice is not remembered.
	}
}
