import type {
	ProgrammingColorComponent,
	ProgrammingColorXyz,
	ProgrammingComponentEdit,
	ProgrammingNativeColorBinding,
	ProgrammingNativeColorEdit,
	ProgrammingScalarEdit,
} from "../../api/generated/light-wire";
import {
	FamilyGestureEditRefusedError,
	type FamilyGestureFamily,
	FamilyGestureSession,
	type FamilyGestureSessionOptions,
} from "./familyGestureSession";

/**
 * Focus, Zoom and Color families for `FamilyGestureSession` (TL-556). Each builder emits only
 * edits the generated `ProgrammingComponentEdit` wire carries and the backend owner accepts
 * (`crates/shared/core/src/programming/edit.rs`):
 *
 * - Focus (`focus`): scalar `focus`, unit domain 0..1 (display 0-100 %).
 * - Zoom (`zoom`): scalar `zoom`, opening degrees 0..180. The backend needs an existing
 *   semantic Zoom value (no default seed); that is a server decision, not refused here.
 * - Color (`color`): Semantic scalar color components, one `coordinates` (XYZ) replacement, or
 *   Direct `native` channel edits. Color wheel selection has no typed wire edit and is absent.
 *
 * Local refusals mirror the backend's structural validation so a forbidden change is reported
 * once and never sent; family coherence beyond that stays a server decision.
 */

export const FOCUS_GESTURE_ATTRIBUTE = "focus";
export const ZOOM_GESTURE_ATTRIBUTE = "zoom";
export const COLOR_GESTURE_ATTRIBUTE = "color";

/** A unit-domain scalar `set` (Focus 0..1) or degrees (Zoom), or color component value. */
export function scalarSet(value: number): ProgrammingScalarEdit {
	return { kind: "set", value: { kind: "value", value } };
}

export function scalarStep(delta: number): ProgrammingScalarEdit {
	return { kind: "relative", value: delta };
}

/**
 * An ordered `[THRU]` spread: the first point lands on the first fixture of the ordered selection
 * (or Group member), the last on the last, interpolated between. The backend resolves it per
 * rank in one transaction; the points keep the order the operator typed.
 */
export function scalarSpread(points: readonly number[]): ProgrammingScalarEdit {
	return { kind: "set", value: { kind: "spread", value: [...points] } };
}

export interface FocusGestureChange {
	/** Focus in the unit domain 0..1. */
	focus?: ProgrammingScalarEdit;
}

export interface ZoomGestureChange {
	/** Zoom opening in degrees, in the value's own beam/field convention. */
	zoom?: ProgrammingScalarEdit;
}

export function focusComponentEdits(
	change: FocusGestureChange,
): ProgrammingComponentEdit[] {
	return change.focus
		? [{ kind: "scalar", component: { kind: "focus" }, operation: change.focus }]
		: [];
}

export function zoomComponentEdits(
	change: ZoomGestureChange,
): ProgrammingComponentEdit[] {
	return change.zoom
		? [{ kind: "scalar", component: { kind: "zoom" }, operation: change.zoom }]
		: [];
}

export interface ColorComponentChange {
	component: ProgrammingColorComponent;
	operation: ProgrammingScalarEdit;
}

export interface NativeColorChange {
	binding: ProgrammingNativeColorBinding;
	operation: ProgrammingNativeColorEdit;
}

/** One Color change sample: Semantic (`coordinates` and/or `components`) or Direct (`native`). */
export interface ColorGestureChange {
	/** Replaces the Semantic base coordinates; exclusive with recipe and hue/saturation edits. */
	coordinates?: ProgrammingColorXyz;
	/** Ordered Semantic component edits; each component at most once. */
	components?: readonly ColorComponentChange[];
	/** Ordered Direct native-channel edits; exclusive with every Semantic edit. */
	native?: readonly NativeColorChange[];
}

const RECIPE: ReadonlySet<ProgrammingColorComponent> = new Set([
	"red",
	"green",
	"blue",
	"amber",
]);
const COORDINATE: ReadonlySet<ProgrammingColorComponent> = new Set([
	"hue",
	"saturation",
]);

function refuse(message: string): never {
	throw new FamilyGestureEditRefusedError(message);
}

function checkColorChange(change: ColorGestureChange) {
	const components = change.components ?? [];
	const native = change.native ?? [];
	const semantic = components.length > 0 || change.coordinates !== undefined;
	if (semantic && native.length > 0)
		refuse("Semantic and Direct Color edits are exclusive");
	if (
		new Set(components.map((entry) => entry.component)).size !==
		components.length
	)
		refuse("duplicate Color component edit");
	const nativeKeys = native.map(
		(entry) => `${entry.binding.channel_id}/${entry.binding.function_id}`,
	);
	if (new Set(nativeKeys).size !== nativeKeys.length)
		refuse("duplicate native Color edit");
	const recipe = components.some((entry) => RECIPE.has(entry.component));
	const coordinate = components.some((entry) => COORDINATE.has(entry.component));
	if (recipe && (coordinate || change.coordinates !== undefined))
		refuse("recipe and coordinate edits cannot write the same Color base");
	if (change.coordinates !== undefined && coordinate)
		refuse("coordinate replacement and component edits are exclusive");
}

/** Builds `[coordinates?, scalar color…]` or `[native…]`; throws for a forbidden mix. */
export function colorComponentEdits(
	change: ColorGestureChange,
): ProgrammingComponentEdit[] {
	checkColorChange(change);
	const edits: ProgrammingComponentEdit[] = [];
	if (change.coordinates)
		edits.push({ kind: "coordinates", xyz: change.coordinates });
	for (const entry of change.components ?? [])
		edits.push({
			kind: "scalar",
			component: { kind: "color", component: entry.component },
			operation: entry.operation,
		});
	for (const entry of change.native ?? [])
		edits.push({
			kind: "native",
			binding: entry.binding,
			operation: entry.operation,
		});
	return edits;
}

export const FOCUS_GESTURE_FAMILY: FamilyGestureFamily<FocusGestureChange> = {
	attribute: FOCUS_GESTURE_ATTRIBUTE,
	buildEdits: focusComponentEdits,
};

export const ZOOM_GESTURE_FAMILY: FamilyGestureFamily<ZoomGestureChange> = {
	attribute: ZOOM_GESTURE_ATTRIBUTE,
	buildEdits: zoomComponentEdits,
};

export const COLOR_GESTURE_FAMILY: FamilyGestureFamily<ColorGestureChange> = {
	attribute: COLOR_GESTURE_ATTRIBUTE,
	buildEdits: colorComponentEdits,
};

/** Focus and Zoom are separate owners: one session (and Undo group) per family. */
export function createFocusGestureSession(options: FamilyGestureSessionOptions) {
	return new FamilyGestureSession(FOCUS_GESTURE_FAMILY, options);
}

export function createZoomGestureSession(options: FamilyGestureSessionOptions) {
	return new FamilyGestureSession(ZOOM_GESTURE_FAMILY, options);
}

export function createColorGestureSession(options: FamilyGestureSessionOptions) {
	return new FamilyGestureSession(COLOR_GESTURE_FAMILY, options);
}
