import { arrayAt, booleanAt, enumAt, recordAt, stringAt } from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

/**
 * TL-554 Direct Color edit options and outcome fields shared by the Normal and Preload values
 * actions. `nativeReference` names the reference head a Direct (`native`) edit adresses;
 * `explicitStart` is the operator's explicit starting colour for the first semantic edit of a
 * Direct value whose visible appearance is unknown (never invented by the desk).
 */

export interface ColorAdoptionInput {
	nativeReference?: { fixtureId: string; headId: string } | null;
	/** A virtual RGB recipe, 0..1 per component; `[0, 0, 0]` is black. */
	explicitStart?: { rgb: readonly [number, number, number] } | null;
}

/** Why a values action was held quietly (no mutation, revision or Undo step). */
export type ProgrammerValuesHold =
	| "displayed_source_unavailable"
	| "native_color_unavailable"
	| "explicit_color_start_required"
	/** TL-637 follow-up: a Zoom edit has no seed in degrees (unknown or unsupported model). */
	| "zoom_unavailable";

export interface ColorAdoptionOutcome {
	fixtures: ReadonlyArray<{
		fixtureId: string;
		start: "approximate" | "explicit";
		uvUnknown: boolean;
	}>;
	limitations: readonly string[];
}

const HOLDS: readonly ProgrammerValuesHold[] = [
	"displayed_source_unavailable",
	"native_color_unavailable",
	"explicit_color_start_required",
	"zoom_unavailable",
];

export function encodeColorAdoption(input: ColorAdoptionInput | null | undefined) {
	const fields: {
		native_reference?: { fixture_id: string; head_id: string };
		explicit_color_start?: { rgb: [number, number, number] };
	} = {};
	if (input?.nativeReference)
		fields.native_reference = {
			fixture_id: input.nativeReference.fixtureId,
			head_id: input.nativeReference.headId,
		};
	if (input?.explicitStart) {
		const rgb = input.explicitStart.rgb;
		if (
			rgb.length !== 3 ||
			rgb.some((value) => !Number.isFinite(value) || value < 0 || value > 1)
		)
			throw new WireValidationError(
				"$.action.explicit_color_start.rgb",
				"three components 0..1",
				rgb,
			);
		fields.explicit_color_start = { rgb: [rgb[0], rgb[1], rgb[2]] };
	}
	return fields;
}

/** Present only when the server held the edit; unknown reasons are ignored. */
export function decodeValuesHold(response: Record<string, unknown>) {
	const hold = HOLDS.find((reason) => reason === response.hold);
	return hold ? { hold } : {};
}

/** Present only with the sample that adopted Direct values semantically. */
export function decodeColorAdoption(
	response: Record<string, unknown>,
): { colorAdoption?: ColorAdoptionOutcome } {
	if (response.color_adoption == null) return {};
	const report = recordAt(response.color_adoption, "$.color_adoption");
	return {
		colorAdoption: {
			fixtures: arrayAt(report.fixtures, "$.color_adoption.fixtures").map(
				(entry, index) => {
					const path = `$.color_adoption.fixtures[${index}]`;
					const fixture = recordAt(entry, path);
					return {
						fixtureId: stringAt(fixture.fixture_id, `${path}.fixture_id`),
						start: enumAt(fixture.start, `${path}.start`, [
							"approximate",
							"explicit",
						] as const),
						uvUnknown: booleanAt(fixture.uv_unknown, `${path}.uv_unknown`),
					};
				},
			),
			limitations: arrayAt(report.limitations, "$.color_adoption.limitations").map(
				(entry, index) =>
					stringAt(entry, `$.color_adoption.limitations[${index}]`),
			),
		},
	};
}
