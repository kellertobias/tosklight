import { useMemo } from "react";
import type {
	FamilyEncoderComponentSlot,
	FamilyEncoderPagesSnapshot,
} from "../../../../api/familyEncoderModels";
import type {
	NativeColorPagesSnapshot,
	NativeColorReferenceRef,
} from "../../../../api/nativeColorModels";
import { useNativeColorPages } from "../../../../features/familyEncoders/useNativeColorPages";
import type { FamilySlotDisplay, ProgrammerValueEntry } from "./familyEncoderDisplay";
import type { FamilyLayoutSlot } from "./familyEncoderLayout";
import {
	currentRaw,
	type NativeRequestedRaw,
	nativeEncoderPages,
	nativeReferenceOf,
	nativeValueText,
} from "./nativeColorSlots";

/**
 * TL-554: Direct Color encoder pages 3/4 for the semantic Color family.
 *
 * Reads the reference head's native pages (an inert read) and composes their slots. Values show
 * the requested Direct recipe of the reference head when the Programmer holds one, otherwise the
 * displayed premaster value. Nothing here sends a request to the Programmer.
 */
export interface NativeColorEncoderPages {
	snapshot: NativeColorPagesSnapshot | null;
	pages: FamilyLayoutSlot[][];
	reference(): NativeColorReferenceRef | null;
	display(slot: FamilyEncoderComponentSlot): FamilySlotDisplay;
}

/** The reference head's requested Direct recipe raws, by channel. */
export function requestedNativeRaw(
	values: readonly ProgrammerValueEntry[],
	reference: NativeColorReferenceRef | null,
): NativeRequestedRaw {
	const raws = new Map<string, number>();
	const entry = reference
		? values.find(
				(value) =>
					value.fixtureId === reference.fixture_id && value.attribute === "color",
			)
		: undefined;
	const value = entry?.value;
	if (value?.kind === "color_program" && value.value.kind === "direct")
		for (const channel of value.value.recipe.channels)
			raws.set(channel.channel_id, channel.raw);
	return raws;
}

/** Re-read the displayed premaster values only when the reference's Semantic value changes. */
function refreshKeyOf(
	values: readonly ProgrammerValueEntry[],
	fixtureIds: readonly string[],
) {
	const color = values.filter(
		(value) => value.attribute === "color" && fixtureIds.includes(value.fixtureId),
	);
	return JSON.stringify(
		color.map((value) =>
			value.value.kind === "color_program" && value.value.value.kind === "direct"
				? [value.fixtureId, "direct"]
				: [value.fixtureId, value.value],
		),
	);
}

export function useNativeColorEncoderPages(
	familySnapshot: FamilyEncoderPagesSnapshot | null,
	values: readonly ProgrammerValueEntry[],
	active: boolean,
): NativeColorEncoderPages {
	const group = familySnapshot?.semantic
		? familySnapshot.families.find((family) => family.family === "color")
		: undefined;
	const fixtureIds = group?.fixture_ids ?? EMPTY;
	const refreshKey = refreshKeyOf(values, fixtureIds);
	const snapshot = useNativeColorPages(
		fixtureIds,
		active && fixtureIds.length > 0,
		refreshKey,
	);
	const reference = nativeReferenceOf(snapshot);
	const requested = useMemo(
		() => requestedNativeRaw(values, reference),
		[values, reference?.fixture_id, reference?.head_id],
	);
	const pages = useMemo(() => {
		const label = snapshot?.reference
			? (snapshot.reference.fixture_number?.toString() ??
				snapshot.reference.fixture_name)
			: "";
		return nativeEncoderPages(snapshot, requested, fixtureIds).map((page) =>
			page.map((slot) =>
				slot?.kind === "component" && label
					? { ...slot, slot: { ...slot.slot, label: `${slot.slot.label} · ${label}` } }
					: slot,
			),
		);
	}, [snapshot, requested, fixtureIds]);
	return {
		snapshot,
		pages,
		reference: () => reference,
		display(slot) {
			if (slot.component.kind !== "native_color")
				return { value: null, text: "—", source: "none" };
			const raw = currentRaw(snapshot, requested, slot.component.component.channel_id);
			if (raw === null) return { value: null, text: "—", source: "none" };
			return {
				value: raw,
				// TL-544 G4: a wheel slot or macro shows its choice.
				text: nativeValueText(slot, raw),
				source: requested.has(slot.component.component.channel_id)
					? "requested"
					: "resolved",
			};
		},
	};
}

const EMPTY: readonly string[] = [];
