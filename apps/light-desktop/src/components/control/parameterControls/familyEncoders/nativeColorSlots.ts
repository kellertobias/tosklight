import type { FamilyEncoderComponentSlot } from "../../../../api/familyEncoderModels";
import type {
	NativeColorControlDescriptor,
	NativeColorFunctionDescriptor,
	NativeColorPagesSnapshot,
	NativeColorReferenceRef,
} from "../../../../api/nativeColorModels";
import type { FamilyLayoutSlot } from "./familyEncoderLayout";

/**
 * Direct (native) Color encoder slots of pages 3/4 (TL-554).
 *
 * Each slot is one native control of the clearly identified reference head, addressed by its
 * channel and function UUIDs (never by its label). A slot edits within the function that holds
 * the control's current value: continuous functions take relative and typed edits in full-width
 * native integers; a discrete function (wheel slot, macro) is shown but not stepped here (its
 * other functions need a complete recipe, so the Color modal offers them as choices only).
 *
 * Pure: navigation, reading values and choosing a reference never send anything.
 */

/** Detents per function range: one detent moves 1/255 of it (an 8-bit step), at least 1. */
const DETENTS_PER_RANGE = 255;

export interface NativeSlotState {
	control: NativeColorControlDescriptor;
	/** The function the control's current value lies in (the first when unknown). */
	function: NativeColorFunctionDescriptor;
	/** Current raw: the requested Direct value, else the displayed premaster value. */
	raw: number | null;
}

export function functionFor(
	control: NativeColorControlDescriptor,
	raw: number | null,
): NativeColorFunctionDescriptor | null {
	const holds = (fn: NativeColorFunctionDescriptor) =>
		raw !== null &&
		raw >= Math.min(fn.raw_from, fn.raw_to) &&
		raw <= Math.max(fn.raw_from, fn.raw_to);
	return control.functions.find(holds) ?? control.functions[0] ?? null;
}

export function nativeStep(fn: NativeColorFunctionDescriptor) {
	const range = Math.abs(fn.raw_to - fn.raw_from);
	return Math.max(1, Math.floor(range / DETENTS_PER_RANGE));
}

/** The family encoder slot of one native control (owner `color`, unit `native_integer`). */
export function nativeColorSlot(
	control: NativeColorControlDescriptor,
	fn: NativeColorFunctionDescriptor,
	fixtureIds: readonly string[],
): FamilyEncoderComponentSlot {
	const min = Math.min(fn.raw_from, fn.raw_to);
	const max = Math.max(fn.raw_from, fn.raw_to);
	return {
		id: control.id,
		label: control.label,
		component: {
			kind: "native_color",
			component: { channel_id: control.channel_id, function_id: fn.function_id },
		},
		descriptor: {
			owner: "color",
			role: "native_color",
			unit: "native_integer",
			domain: { kind: "bounded", bounds: { min, max } },
			step: nativeStep(fn),
			fine_step: 1,
			display_scale: 1,
			interpolation: "linear",
			capability: "verified_native_control",
			spread: false,
			align: false,
			dynamics: false,
		},
		limits: { min, max },
		limits_source: "descriptor",
		convention: null,
		edit: fn.continuous ? "scalar" : "unavailable",
		fixture_ids: [...fixtureIds],
	};
}

export function isNativeSlot(slot: FamilyEncoderComponentSlot | null | undefined) {
	return slot?.component.kind === "native_color";
}

/** The reference a Direct edit names (explicit; the server never guesses another head). */
export function nativeReferenceOf(
	pages: NativeColorPagesSnapshot | null,
): NativeColorReferenceRef | null {
	return pages?.reference
		? { fixture_id: pages.reference.fixture_id, head_id: pages.reference.head_id }
		: null;
}

/** Requested raw values of the reference head's Direct recipe, by channel. */
export type NativeRequestedRaw = ReadonlyMap<string, number>;

export function currentRaw(
	pages: NativeColorPagesSnapshot | null,
	requested: NativeRequestedRaw,
	channelId: string,
): number | null {
	const value = requested.get(channelId);
	if (value !== undefined) return value;
	const shown = pages?.values?.controls.find((entry) => entry.channel_id === channelId);
	return shown ? shown.raw : null;
}

/**
 * Encoder pages 3/4 of the reference head: up to eight slots, four per page, in path order.
 * Returns `[]` without a verified reference (the pages are quietly absent).
 */
export function nativeEncoderPages(
	pages: NativeColorPagesSnapshot | null,
	requested: NativeRequestedRaw,
	fixtureIds: readonly string[],
): FamilyLayoutSlot[][] {
	if (!pages?.semantic || !pages.reference) return [];
	return pages.pages.map((page) =>
		page.controls.map((control) => {
			if (!control) return null;
			const fn = functionFor(control, currentRaw(pages, requested, control.channel_id));
			return fn
				? { kind: "component", family: "color", slot: nativeColorSlot(control, fn, fixtureIds) }
				: null;
		}),
	);
}

/** One typed or stepped edit of a native slot, in full-width native integers. */
export function nativeOperation(
	slot: FamilyEncoderComponentSlot,
	operation: { kind: "relative"; value: number } | { kind: "set"; value: { kind: "value"; value: number } } | { kind: string; value: unknown },
) {
	const limits = slot.limits;
	if (operation.kind === "relative" && typeof operation.value === "number") {
		const delta = Math.trunc(operation.value);
		return delta === 0 ? null : { kind: "relative" as const, value: delta };
	}
	if (operation.kind === "set") {
		const value = (operation.value as { kind?: string; value?: unknown }) ?? {};
		if (value.kind !== "value" || typeof value.value !== "number" || !Number.isFinite(value.value))
			return null;
		const raw = Math.round(value.value);
		const clamped = limits ? Math.min(Math.max(raw, limits.min), limits.max) : raw;
		return { kind: "set" as const, value: clamped };
	}
	return null;
}
