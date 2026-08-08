import type {
	DynamicDefinitionProjection,
	DynamicUpdateIntent,
} from "../../api/types";
import { dynamicSpatialDraft } from "../../features/dynamics/dynamicSpatialDraft";
import type { SpatialProjection } from "../../features/spatialMapping/contracts";
import {
	PROJECTION_KINDS,
	projectionFields,
	projectionKind,
	withProjectionKind,
} from "../../features/spatialMapping/projectionKinds";
import { encoderChoices, type DynamicEncoderSlot } from "./DynamicEncoderDeck";
import { projectionForPreset } from "../groupsWindow/spatialMapping";

/**
 * Two pages: the projection type and where it sits, then how it is oriented.
 *
 * The deck pages by slicing at the encoder width, so the first page is padded out to that
 * width; otherwise the orientation values would climb onto page one at wider layouts and the
 * two pages would stop meaning anything.
 */

const ANGLE = {
	minimum: -3600,
	maximum: 3600,
	inputScale: 1,
	fineStep: 1,
	coarseStep: 15,
};

/** Past the poles the aim flips round to the other side, so elevation stops there. */
const ELEVATION = {
	minimum: -90,
	maximum: 90,
	inputScale: 1,
	fineStep: 1,
	coarseStep: 15,
};

const DISTANCE = {
	minimum: -1000,
	maximum: 1000,
	inputScale: 1,
	fineStep: 0.1,
	coarseStep: 1,
};

function placeholder(index: number): DynamicEncoderSlot {
	return {
		id: `projection-empty-${index}`,
		label: "",
		display: "",
		value: 0,
		disabled: true,
		...DISTANCE,
		apply: async () => undefined,
	};
}

export function projectionEncoderSlots(
	dynamic: DynamicDefinitionProjection,
	encoderWidth: number,
	onMutate: (
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	const mapping = dynamicSpatialDraft(dynamic.spatial_mapping);
	// An inherited projection has no local values to turn, so the first change starts an
	// override from the same default the editor offers.
	const projection: SpatialProjection =
		mapping.projection.type === "replace"
			? mapping.projection.value
			: projectionForPreset("top");
	const kind = projectionKind(projection);

	const write = (next: SpatialProjection, group: string) =>
		onMutate(
			{
				type: "set_spatial_mapping",
				spatial_mapping: {
					...mapping,
					projection: { type: "replace", value: next },
				},
			} as DynamicUpdateIntent,
			group,
		);

	const kindIndex = PROJECTION_KINDS.findIndex(
		(entry) => entry.value === kind,
	);
	const typeSlot: DynamicEncoderSlot = {
		id: "projection-kind",
		label: "Projection",
		display: PROJECTION_KINDS[kindIndex]?.label ?? "Planar",
		value: kindIndex < 0 ? 0 : kindIndex,
		minimum: 0,
		maximum: PROJECTION_KINDS.length - 1,
		inputScale: 0.02,
		fineStep: 1,
		coarseStep: 1,
		choices: encoderChoices(
			"Projection",
			kindIndex < 0 ? 0 : kindIndex,
			PROJECTION_KINDS.map((entry) => ({ label: entry.label })),
		),
		apply: async (value, group) => {
			const entry =
				PROJECTION_KINDS[
					Math.min(
						PROJECTION_KINDS.length - 1,
						Math.max(0, Math.round(value)),
					)
				];
			if (entry) await write(withProjectionKind(projection, entry.value), group);
		},
		selectPreset: async (value, group) => {
			const entry = PROJECTION_KINDS.find(
				(candidate) => candidate.label === value || candidate.value === value,
			);
			if (entry) await write(withProjectionKind(projection, entry.value), group);
		},
	};

	const fields = projectionFields(projection).map((field) => {
		// The deck ramps its step with drag speed, so a raw turn lands on fractions of a degree.
		// Angles are set in whole degrees, which is the only resolution the numbers are read at.
		const angle = field.unit === "°";
		const range = !angle
			? DISTANCE
			: field.key === "elevation"
				? ELEVATION
				: ANGLE;
		const quantize = angle ? Math.round : (value: number) => value;
		return {
			id: `projection-${field.key}`,
			label: field.label,
			display: `${quantize(Math.round(field.value * 100) / 100)}${field.unit ?? ""}`,
			value: quantize(field.value),
			...range,
			apply: async (value: number, group: string) =>
				write(field.apply(quantize(value)), group),
		};
	});

	// Page one places the projection; page two orients it. Planar reads no position, so it has
	// nothing to place and keeps everything on page one.
	const placing = kind === "planar" ? fields : fields.slice(0, 3);
	const orienting = kind === "planar" ? [] : fields.slice(3);
	const first = [typeSlot, ...placing];
	if (!orienting.length) return first;
	const padded = [
		...first,
		...Array.from(
			{ length: Math.max(0, encoderWidth - first.length) },
			(_, index) => placeholder(index),
		),
	];
	return [...padded, ...orienting];
}
