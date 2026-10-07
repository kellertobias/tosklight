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
 * native integers; a discrete function (wheel slot, macro) steps through the control's ordered
 * functions instead (TL-544 G4): each step sends one absolute value inside the next function, which
 * the core adopts as a complete recipe (that channel changes function, every other is kept).
 *
 * Pure: navigation, reading values and choosing a reference never send anything.
 */

/** Detents per function range: one detent moves 1/255 of it (an 8-bit step), at least 1. */
const DETENTS_PER_RANGE = 255;

/**
 * The control descriptor behind each native slot, by channel UUID. Slots are rebuilt and copied
 * (labels), so the ordered functions are looked up by the slot's channel rather than its object.
 */
const CONTROLS = new Map<string, NativeColorControlDescriptor>();

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
	CONTROLS.set(control.channel_id, control);
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
			// TL-544 G6: `[THRU]` spreads a continuous function (the server validates the range).
			spread: fn.continuous,
			align: false,
			dynamics: false,
		},
		limits: { min, max },
		limits_source: "descriptor",
		convention: null,
		// TL-544 G4: a discrete function is edited too, by stepping through the choices.
		edit: "scalar",
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
	if (operation.kind !== "set") return null;
	const value = (operation.value as { kind?: string; value?: unknown }) ?? {};
	const clamp = (raw: number) =>
		limits ? Math.min(Math.max(raw, limits.min), limits.max) : raw;
	if (value.kind === "value" && typeof value.value === "number" && Number.isFinite(value.value))
		return { kind: "set" as const, value: clamp(Math.round(value.value)) };
	// TL-544 G6: an ordered `[THRU]` spread, as rounded full-width integers inside the function.
	if (value.kind === "spread" && Array.isArray(value.value)) {
		const points = value.value as unknown[];
		if (points.length < 2 || !points.every((point) => typeof point === "number" && Number.isFinite(point)))
			return null;
		return {
			kind: "spread" as const,
			value: (points as number[]).map((point) => clamp(Math.round(point))),
		};
	}
	return null;
}

/** TL-544 G4: the ordered choices (functions) of a native slot's control. */
export function nativeChoices(
	slot: FamilyEncoderComponentSlot,
): readonly NativeColorFunctionDescriptor[] {
	if (slot.component.kind !== "native_color") return [];
	return CONTROLS.get(slot.component.component.channel_id)?.functions ?? [];
}

/** The raw a choice is selected with: the start of its function's range. */
export function nativeChoiceRaw(fn: NativeColorFunctionDescriptor) {
	return Math.min(fn.raw_from, fn.raw_to);
}

/** A native slot whose current function is discrete (a wheel slot or macro): it steps choices. */
export function nativeDiscrete(slot: FamilyEncoderComponentSlot) {
	if (slot.component.kind !== "native_color") return false;
	const current = slot.component.component.function_id;
	return nativeChoices(slot).some((fn) => fn.function_id === current && !fn.continuous);
}

/**
 * The next (`1`) or previous (`-1`) choice after `current` (default: the slot's function), in
 * the control's function order, wrapping. `null` when there is no other choice.
 */
export function nativeChoiceStep(
	slot: FamilyEncoderComponentSlot,
	direction: 1 | -1,
	current?: string,
): NativeColorFunctionDescriptor | null {
	if (slot.component.kind !== "native_color") return null;
	const choices = nativeChoices(slot);
	if (choices.length < 2) return null;
	const from = current ?? slot.component.component.function_id;
	const index = choices.findIndex((fn) => fn.function_id === from);
	const next =
		index < 0
			? direction > 0
				? 0
				: choices.length - 1
			: (index + direction + choices.length) % choices.length;
	return choices[next] ?? null;
}

/**
 * The complete native edit of one operation on a native slot. A typed or chosen absolute value
 * names the function that holds it (a function change, TL-544 G4); relative steps and spreads
 * stay inside the slot's current function.
 */
export function nativeEdit(
	slot: FamilyEncoderComponentSlot,
	operation: Parameters<typeof nativeOperation>[1],
) {
	if (slot.component.kind !== "native_color") return null;
	const binding = slot.component.component;
	const value = (operation.value as { kind?: string; value?: unknown } | null) ?? {};
	if (
		operation.kind === "set" &&
		value.kind === "value" &&
		typeof value.value === "number" &&
		Number.isFinite(value.value)
	) {
		const raw = Math.round(value.value);
		const owner = nativeChoices(slot).find(
			(fn) => raw >= Math.min(fn.raw_from, fn.raw_to) && raw <= Math.max(fn.raw_from, fn.raw_to),
		);
		if (owner && owner.function_id !== binding.function_id)
			return {
				binding: { channel_id: binding.channel_id, function_id: owner.function_id },
				operation: { kind: "set" as const, value: raw },
			};
	}
	const native = nativeOperation(slot, operation);
	return native ? { binding, operation: native } : null;
}

/** What a native slot shows: a discrete function's choice label, else the raw value. */
export function nativeValueText(slot: FamilyEncoderComponentSlot, raw: number) {
	const fn = nativeChoices(slot).find(
		(entry) => raw >= Math.min(entry.raw_from, entry.raw_to) && raw <= Math.max(entry.raw_from, entry.raw_to),
	);
	return fn && !fn.continuous ? fn.label : String(raw);
}
