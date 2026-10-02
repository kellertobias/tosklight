import { vi } from "vitest";
import type {
	FamilyEncoderComponentSlot,
	FamilyEncoderGroup,
	FamilyEncoderPagesSnapshot,
	FamilyEncoderSlot,
	ProgrammingComponent,
	ProgrammingComponentDescriptor,
} from "../../../../api/familyEncoderModels";
import type { FamilyGestureTimers } from "../../../../features/programmerValues/familyGestureSession";

/** Shared fixtures for the family encoder foundation tests (not production code). */

export const FIXTURE_A = "11111111-1111-4111-8111-111111111111";
export const FIXTURE_B = "22222222-2222-4222-8222-222222222222";

function descriptor(
	owner: ProgrammingComponentDescriptor["owner"],
	unit: ProgrammingComponentDescriptor["unit"],
	step: number,
	domain: ProgrammingComponentDescriptor["domain"],
): ProgrammingComponentDescriptor {
	return {
		owner,
		role: owner === "position" ? "angle" : owner === "color" ? "color_recipe" : owner,
		unit,
		domain,
		step,
		fine_step: step / 10,
		display_scale: unit === "percent" ? 100 : 1,
		interpolation: "linear",
		capability: owner === "focus" ? "focus_parameter" : "semantic_intent",
		spread: domain !== null,
		align: domain !== null,
		dynamics: domain !== null,
	} as ProgrammingComponentDescriptor;
}

const UNIT = { kind: "bounded", bounds: { min: 0, max: 1 } } as const;

export function componentSlot(
	id: string,
	component: ProgrammingComponent,
	overrides: Partial<FamilyEncoderComponentSlot> = {},
): FamilyEncoderComponentSlot {
	const kind = component.kind;
	const base: Record<string, () => ProgrammingComponentDescriptor> = {
		pan: () => descriptor("position", "degrees", 1, { kind: "finite" }),
		tilt: () => descriptor("position", "degrees", 1, { kind: "finite" }),
		target_reference: () => descriptor("position", "selection", 1, null),
		target_x: () => descriptor("position", "metres", 0.1, { kind: "finite" }),
		target_y: () => descriptor("position", "metres", 0.1, { kind: "finite" }),
		target_z: () => descriptor("position", "metres", 0.1, { kind: "finite" }),
		focus: () => descriptor("focus", "percent", 0.01, UNIT),
		zoom: () =>
			descriptor("zoom", "degrees", 1, {
				kind: "bounded",
				bounds: { min: 0, max: 180 },
			}),
		color: () => descriptor("color", "percent", 0.01, UNIT),
		color_wheel: () => descriptor("color", "selection", 1, null),
	};
	return {
		id,
		label: id,
		component,
		descriptor: (base[kind] ?? base.color)(),
		limits: null,
		limits_source: "unknown",
		convention: null,
		edit:
			kind === "target_reference"
				? "target_reference"
				: kind === "color_wheel"
					? "unavailable"
					: "scalar",
		fixture_ids: [FIXTURE_A, FIXTURE_B],
		...overrides,
	};
}

const asSlot = (slot: FamilyEncoderComponentSlot): FamilyEncoderSlot => ({
	kind: "component",
	...slot,
});

export const PAN = componentSlot("position.pan", { kind: "pan" });
export const TILT = componentSlot("position.tilt", { kind: "tilt" });
export const POINT = componentSlot("position.target", { kind: "target_reference" });
export const TARGET_X = componentSlot("position.target.x", { kind: "target_x" });
export const TARGET_Y = componentSlot("position.target.y", { kind: "target_y" });
export const TARGET_Z = componentSlot("position.target.z", { kind: "target_z" });
export const FOCUS = componentSlot("focus", { kind: "focus" });
export const ZOOM = componentSlot("zoom", { kind: "zoom" }, { convention: "beam" });
/** A Zoom whose selection publishes no beam/field convention (unknown or mixed). */
export const ZOOM_UNKNOWN_CONVENTION = componentSlot("zoom", { kind: "zoom" });
export const RED = componentSlot("color.red", { kind: "color", component: "red" });
export const WHEEL = componentSlot("color.wheel.1", { kind: "color_wheel", component: 0 });

function group(
	family: FamilyEncoderGroup["family"],
	pages: Array<Array<FamilyEncoderComponentSlot | FamilyEncoderSlot | null>>,
	extra: Partial<FamilyEncoderGroup> = {},
): FamilyEncoderGroup {
	return {
		family,
		owners: [],
		fixture_ids: [FIXTURE_A, FIXTURE_B],
		replaces_attributes: [],
		replaces_attribute_prefixes: [],
		pages: pages.map((slots, index) => ({
			number: index + 1,
			label: `${family} ${index + 1}`,
			slots: slots.map((slot) =>
				slot === null ? null : "kind" in slot && (slot.kind === "attribute" || slot.kind === "component") ? (slot as FamilyEncoderSlot) : asSlot(slot as FamilyEncoderComponentSlot),
			),
		})),
		reserved_pages: [],
		...extra,
	};
}

export function pagesSnapshot(semantic: boolean): FamilyEncoderPagesSnapshot {
	return {
		semantic,
		supported_programming_contract: semantic ? 1 : 0,
		semantic_programming_contract: 1,
		color_presentation: "easy_rgbw",
		show_revision: 1,
		fixture_ids: [FIXTURE_A, FIXTURE_B],
		families: [
			group("position", [[PAN, TILT, null, null], [POINT, TARGET_X, TARGET_Y, TARGET_Z]], {
				replaces_attributes: ["pan", "tilt"],
			}),
			group("color", [[RED, null, null, null]], {
				replaces_attributes: ["color"],
				replaces_attribute_prefixes: ["color."],
				reserved_pages: [{ number: 3, reason: "native_color" }],
			}),
			group(
				"focus",
				[[FOCUS, ZOOM, { kind: "attribute", attribute: "softness", label: "Softness" }, null]],
				{ replaces_attributes: ["focus", "zoom", "softness"] },
			),
		],
	};
}

/** A Normal or Preload writer that records every call. */
export function fakeWriter() {
	return {
		applyIntent: vi.fn(async (_input: unknown) => ({ status: "applied" })),
		cancelGesture: vi.fn((_undoGroup: string) => 0),
		finishGesture: vi.fn(async (_input: unknown) => ({ status: "applied" })),
	};
}

/** Manual timers for the idle-end encoder mode. */
export function manualTimers() {
	let next = 0;
	const pending = new Map<number, () => void>();
	const timers: FamilyGestureTimers = {
		setTimeout: (callback) => {
			next += 1;
			pending.set(next, callback);
			return next;
		},
		clearTimeout: (handle) => {
			pending.delete(handle as number);
		},
	};
	return {
		timers,
		fireAll() {
			const callbacks = [...pending.values()];
			pending.clear();
			for (const callback of callbacks) callback();
		},
		get pending() {
			return pending.size;
		},
	};
}

export function sequentialIds() {
	let id = 0;
	return () => `id-${++id}`;
}
