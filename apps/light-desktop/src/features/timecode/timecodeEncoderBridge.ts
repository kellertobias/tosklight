import { useSyncExternalStore } from "react";

export interface TimecodeEncoderSlot {
	id: string;
	label: string;
	display: string;
	value: number;
	minimum: number;
	maximum: number;
	fineStep: number;
	coarseStep: number;
	disabled?: boolean;
	set(value: number): void;
}

export interface TimecodeEncoderDeck {
	timeline: readonly TimecodeEncoderSlot[];
	keyframe: readonly TimecodeEncoderSlot[];
	selectionLabel?: "Selected Cue" | "Selected Keyframe" | "Selected Marker";
}

let active: { owner: symbol; deck: TimecodeEncoderDeck } | null = null;
const listeners = new Set<() => void>();

export function publishTimecodeEncoderDeck(
	owner: symbol,
	deck: TimecodeEncoderDeck,
): void {
	active = { owner, deck };
	for (const listener of listeners) listener();
}

export function clearTimecodeEncoderDeck(owner: symbol): void {
	if (active?.owner !== owner) return;
	active = null;
	for (const listener of listeners) listener();
}

export function useTimecodeEncoderDeck(): TimecodeEncoderDeck | null {
	return useSyncExternalStore(
		(listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		() => active?.deck ?? null,
		() => null,
	);
}
