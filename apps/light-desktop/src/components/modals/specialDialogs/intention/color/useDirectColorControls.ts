import { useEffect, useMemo, useRef, useState } from "react";
import type { FamilyEncoderComponentSlot } from "../../../../../api/familyEncoderModels";
import type {
	NativeColorPagesSnapshot,
	NativeColorReferenceCandidate,
} from "../../../../../api/nativeColorModels";
import { colorAdoptionNotice } from "../../../../../features/familyEncoders/colorAdoptionNotice";
import { useFamilyEncodersContext } from "../../../../../features/familyEncoders/FamilyEncodersProvider";
import { nativeColorReference } from "../../../../../features/familyEncoders/nativeColorReference";
import { useNativeColorPages } from "../../../../../features/familyEncoders/useNativeColorPages";
import { FamilyEncoderBinding } from "../../../../control/parameterControls/familyEncoders/familyEncoderBinding";
import {
	currentRaw,
	functionFor,
	nativeChoiceRaw,
	nativeColorSlot,
	nativeReferenceOf,
} from "../../../../control/parameterControls/familyEncoders/nativeColorSlots";
import { requestedNativeRaw } from "../../../../control/parameterControls/familyEncoders/useNativeColorEncoderPages";
import { familyGestureWriter } from "../../../../control/parameterControls/familyEncoders/useFamilyEncoderBinding";
import type { ColorDialogLane } from "./useColorDialogLane";

/** One native overflow control of the reference head, ready for a touch encoder. */
export interface DirectColorControl {
	slot: FamilyEncoderComponentSlot;
	/** The control's current raw: the requested recipe, else the displayed premaster value. */
	raw: number | null;
	/**
	 * The control's functions as choices (TL-544 G4): choosing one sets its first raw, a complete
	 * recipe adoption; only the current one is adjustable in place.
	 */
	choices: ReadonlyArray<{ label: string; current: boolean; raw: number }>;
}

export interface DirectColorControls {
	pages: NativeColorPagesSnapshot | null;
	overflow: readonly DirectColorControl[];
	step(control: DirectColorControl, delta: number): void;
	set(control: DirectColorControl, value: number): void;
	/** Ordered `[THRU]` raw points over the selection; continuous functions only (TL-544 G6). */
	setRange(control: DirectColorControl, points: readonly number[]): void;
	/** Explicit release of a touch drag: finishes the open Direct gesture now (TL-544 G12). */
	finishGestures(): void;
	/** Inert: chooses the inspected reference head; sends nothing to the Programmer. */
	chooseReference(candidate: NativeColorReferenceCandidate): void;
}

/**
 * TL-554 Direct section of the full Color modal: the reference head, the native controls that
 * do not fit on encoder pages 3/4 (overflow) and their edits. Overflow edits use the same
 * family encoder binding as pages 3/4 (idle-end gestures, one Undo group, explicit reference),
 * so the modal, the software encoders and hardware/OSC encoders send identical Direct edits.
 */
export function useDirectColorControls(
	lane: ColorDialogLane,
	open: boolean,
): DirectColorControls {
	const context = useFamilyEncodersContext();
	const refreshKey = useMemo(
		() =>
			JSON.stringify(
				lane.values
					.filter((value) => value.attribute === "color")
					.map((value) =>
						value.value.kind === "color_program" && value.value.value.kind === "direct"
							? [value.fixtureId, "direct"]
							: [value.fixtureId, value.value],
					),
			),
		[lane.values],
	);
	const pages = useNativeColorPages(lane.colorFixtureIds, open, refreshKey);
	const reference = nativeReferenceOf(pages);
	const requested = useMemo(
		() => requestedNativeRaw(lane.values, reference),
		[lane.values, reference?.fixture_id, reference?.head_id],
	);
	const latest = useRef({ lane, context, reference });
	latest.current = { lane, context, reference };
	const [binding, setBinding] = useState<FamilyEncoderBinding | null>(null);
	useEffect(() => {
		if (!open) return;
		const created = new FamilyEncoderBinding({
			writerFor: (name) => familyGestureWriter(latest.current.lane.writers[name]),
			displayedSource: (name) =>
				latest.current.context?.readouts.displayedSource(name) ?? null,
			nativeReference: () => latest.current.reference,
			onColorHold: (reason) => colorAdoptionNotice.held(reason),
			onColorOutcome: (outcome) => colorAdoptionNotice.outcome(outcome),
			onError: () => undefined,
		});
		// Window blur and a hidden document end the open Direct gesture (TL-544 G12).
		const detachGuards = created.attachWindowGuards();
		setBinding(created);
		return () => {
			detachGuards();
			created.dispose();
			setBinding((current) => (current === created ? null : current));
		};
	}, [open]);
	const overflow = useMemo(
		() =>
			(pages?.overflow ?? []).flatMap((control) => {
				const raw = currentRaw(pages, requested, control.channel_id);
				const fn = functionFor(control, raw);
				if (!fn) return [];
				return [
					{
						slot: nativeColorSlot(control, fn, lane.colorFixtureIds),
						raw,
						choices:
							control.functions.length > 1 || !fn.continuous
								? control.functions.map((entry) => ({
										label: entry.label,
										current: entry.function_id === fn.function_id,
										raw: nativeChoiceRaw(entry),
									}))
								: [],
					},
				];
			}),
		[pages, requested, lane.colorFixtureIds],
	);
	const target = () =>
		lane.lane ? { lane: lane.lane, groupId: lane.groupId, timing: lane.timing } : null;
	return {
		pages,
		overflow,
		step(control, delta) {
			const edit = target();
			if (edit) binding?.step(control.slot, delta, edit);
		},
		set(control, value) {
			const edit = target();
			if (edit) binding?.set(control.slot, value, edit);
		},
		setRange(control, points) {
			const edit = target();
			if (edit) binding?.spread(control.slot, points, edit);
		},
		finishGestures() {
			binding?.finishGestures();
		},
		chooseReference(candidate) {
			nativeColorReference.set({
				fixtureId: candidate.fixture_id,
				headId: candidate.head_id,
			});
		},
	};
}
