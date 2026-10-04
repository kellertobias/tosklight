import { useEffect, useMemo, useRef, useState } from "react";
import type { FamilyEncoderComponentSlot } from "../../../../api/familyEncoderModels";
import {
	useFamilyEncoderPages,
	useFamilyEncodersContext,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { useFamilyReadouts } from "../../../../features/familyEncoders/useFamilyReadouts";
import { usePatchedFixtures } from "../../../../features/patch/PatchState";
import type { FamilyGestureWriter } from "../../../../features/programmerValues/familyGestureSession";
import { useProgrammerPreloadValuesActions } from "../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerValuesActions } from "../../../../features/programmerValues/ProgrammerValuesView";
import type { ParameterFamily } from "../model";
import {
	immediateParameterTiming,
	type ParameterValuesMutationPort,
	parameterValueTiming,
} from "../parameterValueMutations";
import type { ParameterProjection } from "../useParameterProjection";
import {
	FamilyEncoderBinding,
	type FamilyEncoderTarget,
} from "./familyEncoderBinding";
import {
	type FamilySlotDisplay,
	familySlotDisplay,
	positionSelectionState,
	positionSlotUnsupported,
	type ProgrammerValueEntry,
} from "./familyEncoderDisplay";
import {
	type FamilyLayout,
	familyEncoderLayout,
	familyLayoutSlots,
	SEMANTIC_PARAMETER_FAMILIES,
} from "./familyEncoderLayout";
import { colorAdoptionNotice } from "../../../../features/familyEncoders/colorAdoptionNotice";
import {
	type FamilyPointChoice,
	familyPointChoices,
	pointSlotDisplay,
} from "./familyPointChoices";
import { isNativeSlot } from "./nativeColorSlots";
import {
	type NativeColorEncoderPages,
	useNativeColorEncoderPages,
} from "./useNativeColorEncoderPages";

/**
 * Mounts the semantic family encoder binding for the parameter controller.
 *
 * Inactive (`active: false`, `overrides: {}`) whenever the backend does not report the semantic
 * contract or the family has no semantic layout: the legacy normalized pages, slots and writes
 * are then untouched. Active, it replaces the family's pages with the published semantic layout
 * and routes component slots (software and hardware/OSC) into family gesture sessions.
 */

/** Adapts a lane's mounted writer to the gesture session; null refuses the gesture quietly. */
export function familyGestureWriter(
	port: ParameterValuesMutationPort | null | undefined,
): FamilyGestureWriter | null {
	const applyIntent = port?.applyIntent?.bind(port);
	const finishGesture = port?.finishGesture?.bind(port);
	if (!applyIntent || !finishGesture) return null;
	return {
		applyIntent: (input) => applyIntent(input),
		cancelGesture: (undoGroup) => port?.cancelGesture?.(undoGroup) ?? 0,
		finishGesture: (input) => finishGesture(input),
	};
}

export type FamilyComponentLayoutSlot = {
	kind: "component";
	slot: FamilyEncoderComponentSlot;
};

export interface FamilyEncoderController {
	/** The backend reports the semantic programming contract. */
	semantic: boolean;
	/** A semantic layout is in force for the current family. */
	active: boolean;
	page: number;
	pageCountFor(family: ParameterFamily): number | null;
	selectPage(family: ParameterFamily, page: number): void;
	componentSlot(index: number): FamilyEncoderComponentSlot | null;
	/** Hardware/OSC `encode/N` detent; false when the slot is not semantic. */
	detent(index: number, value: string | undefined): boolean;
	step(index: number, delta: number): void;
	set(index: number, value: number): void;
	/** Ordered `[THRU]` points in descriptor units; ignored by a slot without `spread`. */
	setRange(index: number, points: readonly number[]): void;
	/** The Point slot's ordered choices (Origin, then the show's 3D Points). */
	pointChoices: readonly FamilyPointChoice[];
	/** Picks one Point choice by its value on the Point slot: one complete Target gesture. */
	choosePoint(index: number, value: string): void;
	/**
	 * Explicit release of a software encoder drag: finishes the open encoder gestures now (one
	 * Finish, same Undo group) instead of waiting for the idle end (TL-544 G12).
	 */
	finishGestures(): void;
	display(index: number): FamilySlotDisplay;
	/** Controller fields replaced while active; `{}` keeps the legacy controller byte-for-byte. */
	overrides: Partial<
		Pick<
			ParameterProjection,
			"encoderPage" | "encoderPageCount" | "encoderSlots" | "encoderPushTurnSlots"
		>
	>;
}

function useFamilyLayouts(projection: ParameterProjection): {
	semantic: boolean;
	layouts: Map<ParameterFamily, FamilyLayout>;
	native: NativeColorEncoderPages;
} {
	const snapshot = useFamilyEncoderPages(
		projection.selectedFixtureIds,
		projection.active,
	);
	// TL-554: Direct Color pages 3/4 of the reference head (an inert read).
	const native = useNativeColorEncoderPages(
		snapshot,
		projection.programmerValues as readonly ProgrammerValueEntry[],
		projection.active,
	);
	const supported = projection.supportedFixtureIdsByAttribute;
	const layouts = useMemo(() => {
		const layouts = new Map<ParameterFamily, FamilyLayout>();
		for (const family of Object.keys(SEMANTIC_PARAMETER_FAMILIES) as ParameterFamily[]) {
			const layout = familyEncoderLayout({
				snapshot,
				family,
				registryGroup: projection.encoderGroups.find(
					(group) => group.id === family.toLowerCase(),
				),
				visibleEncoderCount: projection.visibleEncoderCount,
				supportsAttribute: (attribute) => supported.has(attribute),
				...(family === "Color" ? { nativePages: native.pages } : {}),
			});
			if (layout) layouts.set(family, layout);
		}
		return layouts;
	}, [snapshot, projection.encoderGroups, projection.visibleEncoderCount, supported, native.pages]);
	return { semantic: snapshot?.semantic === true, layouts, native };
}

function useBindingInstance(
	projection: ParameterProjection,
	readouts: ReturnType<typeof useFamilyReadouts>,
	native: NativeColorEncoderPages,
	points: readonly FamilyPointChoice[],
) {
	const normal = useProgrammerValuesActions();
	const preload = useProgrammerPreloadValuesActions();
	const context = useFamilyEncodersContext();
	const latest = useRef({ normal, preload, readouts, context, native, points });
	latest.current = { normal, preload, readouts, context, native, points };
	const [binding, setBinding] = useState<FamilyEncoderBinding | null>(null);
	useEffect(() => {
		if (!projection.active) return;
		const next = new FamilyEncoderBinding({
			writerFor: (lane) =>
				familyGestureWriter(
					(lane === "preload" ? latest.current.preload : latest.current.normal) as
						| ParameterValuesMutationPort
						| null,
				),
			// The covering lease of the edited slot's fixtures, never merely the newest.
			displayedSource: (lane, fixtureIds) =>
				latest.current.context?.readouts.displayedSource(lane, fixtureIds) ?? null,
			onDisplayedSourceHold: () => latest.current.readouts.reread(),
			nativeReference: () => latest.current.native.reference(),
			pointChoices: () => latest.current.points.map((choice) => choice.reference),
			semanticColorAdoption: () => colorAdoptionNotice.semanticInput(),
			onColorHold: (reason) => colorAdoptionNotice.held(reason),
			onColorOutcome: (outcome) => colorAdoptionNotice.outcome(outcome),
		});
		// Window blur and a hidden document end the open encoder gestures (TL-544 G12).
		const detachGuards = next.attachWindowGuards();
		setBinding(next);
		return () => {
			detachGuards();
			next.dispose();
			setBinding((current) => (current === next ? null : current));
		};
	}, [projection.active]);
	return binding;
}

function editTarget(
	projection: ParameterProjection,
	positionFixtures: readonly string[],
): FamilyEncoderTarget | null {
	const lane = projection.programmerValuesRoute;
	if (!lane || !projection.programmerValuesReady) return null;
	const position = positionSelectionState(
		projection.programmerValues as readonly ProgrammerValueEntry[],
		positionFixtures,
	);
	return {
		lane,
		groupId: projection.selectedGroupId,
		timing:
			lane === "preload"
				? parameterValueTiming(projection.programmerFadeMillis)
				: immediateParameterTiming(),
		positionRepresentation: position.representation,
		positionTargetReference: position.reference,
	};
}

export function useFamilyEncoderBinding(
	projection: ParameterProjection,
	family: ParameterFamily,
): FamilyEncoderController {
	const { semantic, layouts, native } = useFamilyLayouts(projection);
	const [pages, setPages] = useState<Partial<Record<ParameterFamily, number>>>({});
	const layout = layouts.get(family) ?? null;
	const positionFixtures =
		layouts.get("Position")?.group.fixture_ids ?? EMPTY;
	const readouts = useFamilyReadouts(
		projection.programmerValuesRoute ?? "normal",
		positionFixtures,
		{ enabled: layout?.family === "position", consumerId: "position-encoders" },
	);
	const patchFixtures = usePatchedFixtures(layout?.family === "position");
	const points = useMemo(() => familyPointChoices(patchFixtures), [patchFixtures]);
	const binding = useBindingInstance(projection, readouts, native, points);
	const page = layout
		? Math.min(Math.max(pages[family] ?? 1, 1), layout.pages.length)
		: 1;
	const slots = layout ? familyLayoutSlots(layout, page) : null;
	const componentSlot = (index: number) =>
		slots?.componentSlots[index]?.slot ?? null;
	const target = () => editTarget(projection, positionFixtures);
	const withSlot = (
		index: number,
		run: (slot: FamilyEncoderComponentSlot, target: FamilyEncoderTarget) => void,
	) => {
		const slot = componentSlot(index);
		const edit = target();
		if (!slot || !edit || !binding) return Boolean(slot);
		const positionUnsupported = positionSlotUnsupported(slot, {
			programmerValues: projection.programmerValues as readonly ProgrammerValueEntry[],
			readouts: readouts.snapshot,
		});
		run(slot, positionUnsupported ? { ...edit, positionUnsupported } : edit);
		return true;
	};
	return {
		semantic,
		active: Boolean(layout),
		page,
		pageCountFor: (name) => layouts.get(name)?.pages.length ?? null,
		selectPage: (name, next) => {
			if (layouts.has(name)) setPages((current) => ({ ...current, [name]: next }));
		},
		componentSlot,
		detent: (index, value) =>
			withSlot(index, (slot, edit) => binding?.detent(slot, value, edit)),
		step: (index, delta) => {
			withSlot(index, (slot, edit) => binding?.step(slot, delta, edit));
		},
		set: (index, value) => {
			withSlot(index, (slot, edit) => binding?.set(slot, value, edit));
		},
		setRange: (index, values) => {
			withSlot(index, (slot, edit) => binding?.spread(slot, values, edit));
		},
		pointChoices: points,
		choosePoint: (index, value) => {
			const choice = points.find((entry) => entry.value === value);
			if (choice)
				withSlot(index, (slot, edit) => binding?.chooseTarget(slot, choice.reference, edit));
		},
		finishGestures: () => {
			binding?.finishGestures();
		},
		display: (index) => {
			const slot = componentSlot(index);
			if (isNativeSlot(slot) && slot) return native.display(slot);
			if (slot?.edit === "target_reference") {
				const values = projection.programmerValues as readonly ProgrammerValueEntry[];
				const shown = pointSlotDisplay(slot, values, points);
				const unsupported =
					shown.source === "none" &&
					positionSlotUnsupported(slot, { programmerValues: values, readouts: readouts.snapshot });
				return unsupported ? { ...shown, unsupported: true } : shown;
			}
			return slot
				? familySlotDisplay(slot, {
						programmerValues: projection.programmerValues as readonly ProgrammerValueEntry[],
						readouts: readouts.snapshot,
					})
				: { value: null, text: "—", source: "none" };
		},
		overrides:
			layout && slots
				? {
						encoderPage: page,
						encoderPageCount: layout.pages.length,
						encoderSlots: slots.encoderSlots,
						encoderPushTurnSlots: slots.encoderPushTurnSlots,
					}
				: {},
	};
}

const EMPTY: readonly string[] = [];
