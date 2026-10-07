import {
	useCallback,
	useLayoutEffect,
	useState,
	useSyncExternalStore,
} from "react";
import { encoderAreaFits } from "../../modals/specialDialogs/intention/color/colorDialogModel";

/**
 * The lower encoder area as a placement budget (TL-550).
 *
 * `ParameterControlView` measures its actual `.parameter-surfaces` container (never the viewport)
 * and publishes it here. The Color Special Dialog, which is rendered by the global
 * `SpecialDialogsModal`, reads the budget: it renders compact inside that container when the
 * measured area is at least 680×210, and as the full modal otherwise. While it is inline it
 * claims the area, so the encoder surfaces step aside, and the view routes a press of the active
 * Color family tab or of Special Dialog back to it.
 */

export interface EncoderAreaSize {
	width: number;
	height: number;
}

export interface EncoderAreaState {
	element: HTMLElement | null;
	size: EncoderAreaSize;
	/** The inline dialog currently occupying the area, if any. */
	inline: string | null;
	/** Incremented by each Special Dialog press while the inline dialog is open. */
	cycle: number;
}

const INITIAL: EncoderAreaState = {
	element: null,
	size: { width: 0, height: 0 },
	inline: null,
	cycle: 0,
};

let state = INITIAL;
const listeners = new Set<() => void>();

function update(next: Partial<EncoderAreaState>) {
	const merged = { ...state, ...next };
	if (
		merged.element === state.element &&
		merged.size.width === state.size.width &&
		merged.size.height === state.size.height &&
		merged.inline === state.inline &&
		merged.cycle === state.cycle
	)
		return;
	state = merged;
	for (const listener of listeners) listener();
}

export const encoderAreaStore = {
	get: () => state,
	subscribe(listener: () => void) {
		listeners.add(listener);
		return () => listeners.delete(listener);
	},
	publish(element: HTMLElement | null, size: EncoderAreaSize) {
		update({ element, size });
	},
	/** Removes `element` only if it is still the published area (unmount order is not fixed). */
	withdraw(element: HTMLElement) {
		if (state.element === element)
			update({ element: null, size: { width: 0, height: 0 }, inline: null });
	},
	claim(owner: string) {
		update({ inline: owner });
	},
	release(owner: string) {
		if (state.inline === owner) update({ inline: null });
	},
	requestCycle() {
		update({ cycle: state.cycle + 1 });
	},
	/** Test seam. */
	reset() {
		state = INITIAL;
		for (const listener of listeners) listener();
	},
};

export function useEncoderAreaState() {
	return useSyncExternalStore(
		encoderAreaStore.subscribe,
		encoderAreaStore.get,
		encoderAreaStore.get,
	);
}

/** The published area and whether the compact Color dialog fits it. */
export function useEncoderAreaBudget() {
	const area = useEncoderAreaState();
	return {
		element: area.element,
		fits: Boolean(area.element) && encoderAreaFits(area.size),
		inline: area.inline,
		cycle: area.cycle,
	};
}

/**
 * Measures the element the returned ref is attached to and publishes it as the desk's lower
 * encoder area. The budget follows the container's content box through a ResizeObserver.
 */
export function useEncoderArea<T extends HTMLElement = HTMLDivElement>() {
	const [element, setElement] = useState<T | null>(null);
	const ref = useCallback((node: T | null) => setElement(node), []);
	useLayoutEffect(() => {
		if (!element) return;
		const measure = (width: number, height: number) =>
			encoderAreaStore.publish(element, { width, height });
		const rect = element.getBoundingClientRect();
		measure(rect.width, rect.height);
		const observer =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(([entry]) => {
						if (entry)
							measure(entry.contentRect.width, entry.contentRect.height);
					});
		observer?.observe(element);
		return () => {
			observer?.disconnect();
			encoderAreaStore.withdraw(element);
		};
	}, [element]);
	return { ref, element };
}
