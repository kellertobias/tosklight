import { useSyncExternalStore } from "react";
import type { NativeColorReferenceChoice } from "../../api/nativeColorPagesWire";

/**
 * TL-554: the operator's reference-head choice for the Direct Color pages, shared by the
 * encoders and the Color modal of this desk window. Session-only and desk-local: never stored
 * in the show, the Programmer or Undo history, and choosing one sends nothing. A choice that is
 * no longer part of the selection is ignored by the server (it falls back to the first verified
 * head and reports `chosen: false`).
 */
let current: NativeColorReferenceChoice | null = null;
const listeners = new Set<() => void>();

export const nativeColorReference = {
	get: () => current,
	set(next: NativeColorReferenceChoice | null) {
		if (
			current?.fixtureId === next?.fixtureId &&
			(current?.headId ?? null) === (next?.headId ?? null)
		)
			return;
		current = next;
		for (const listener of listeners) listener();
	},
	subscribe(listener: () => void) {
		listeners.add(listener);
		return () => listeners.delete(listener);
	},
};

export function useNativeColorReferenceChoice() {
	return useSyncExternalStore(
		nativeColorReference.subscribe,
		nativeColorReference.get,
		nativeColorReference.get,
	);
}
