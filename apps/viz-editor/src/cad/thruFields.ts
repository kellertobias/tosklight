/**
 * The fields a selection of several elements is edited through.
 *
 * Every selection shares the placement fields: where the elements stand, how they are turned, and,
 * for lamps, the bracket and barn doors. When every selected element was patched from the *same
 * model*, the selection also carries that model's own controls — the measurements its profile lets
 * the operator set, or a placed model's scale — so four identical stage elements are raised
 * together rather than one at a time.
 *
 * Sameness is the profile id, not a family resemblance: four 2 × 1 m stage elements are one model,
 * a 2 × 1 m beside a 1 × 1 m is not, however alike their adjustable axes look.
 */
import type {
	FixtureProfileScenery,
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import { clampToRange, hasAdjustableSize, placedSize, SIZE_AXES } from "./sceneryAxes";

export interface ThruFieldSpec {
	id: string;
	label: string;
	ariaLabel: string;
	digits: number;
	/** The unit a mixed field names its range in. */
	unit: string;
	/** Only a lamp has it; Venue objects in the selection are left out of the spread. */
	lampsOnly?: boolean;
	/** Empty is a value of its own — no barn doors fitted — rather than a refused edit. */
	optional?: boolean;
	read(fixture: PatchFixtureProjection): number | null;
	write(fixture: PatchFixtureProjection, value: number | null): PatchFixtureProjection;
}

const AXES = ["x", "y", "z"] as const;

/** What any selection is placed by, whatever its elements are. */
export const THRU_FIELDS: readonly ThruFieldSpec[] = [
	...AXES.map(
		(axis): ThruFieldSpec => ({
			id: `position-${axis}`,
			label: `${axis.toUpperCase()} (m)`,
			ariaLabel: `Position ${axis.toUpperCase()}`,
			digits: 3,
			unit: "m",
			read: (fixture) => fixture.location[axis] / 1000,
			write: (fixture, metres) => ({
				...fixture,
				location: { ...fixture.location, [axis]: Math.round((metres ?? 0) * 1000) },
			}),
		}),
	),
	...AXES.map(
		(axis): ThruFieldSpec => ({
			id: `rotation-${axis}`,
			label: `Rot ${axis.toUpperCase()} (°)`,
			ariaLabel: `Rotation ${axis.toUpperCase()}`,
			digits: 1,
			unit: "°",
			read: (fixture) => fixture.rotation[axis],
			write: (fixture, degrees) => ({
				...fixture,
				rotation: { ...fixture.rotation, [axis]: degrees ?? 0 },
			}),
		}),
	),
	{
		id: "bracket",
		label: "Bracket angle (°)",
		ariaLabel: "Bracket angle",
		digits: 1,
		unit: "°",
		lampsOnly: true,
		read: (fixture) => fixture.bracketAngle ?? 0,
		write: (fixture, degrees) => ({ ...fixture, bracketAngle: degrees ?? 0 }),
	},
	{
		id: "barndoors",
		label: "Barndoors (°)",
		ariaLabel: "Barndoors",
		digits: 1,
		unit: "°",
		lampsOnly: true,
		optional: true,
		read: (fixture) => fixture.shaperAngle ?? null,
		write: (fixture, degrees) => ({ ...fixture, shaperAngle: degrees }),
	},
];

/** The one model a whole selection was patched from. */
export interface SharedModel {
	profileId: string;
	/** What the panel calls it: the manufacturer and name of the profile, when the patch knows them. */
	label: string;
	/** What the profile generates, when it generates anything. */
	scenery: FixtureProfileScenery | null;
	/** A crowd area is drawn at the size it is given and never scaled. */
	crowd: boolean;
}

/**
 * The model every selected element was patched from, or null when they are not all the same.
 *
 * The revisions the selection was built from may differ — one element updated to a corrected
 * profile, its neighbour not — and the controls still belong to the model, so the panel resolves
 * the revision the first selected element stands on and offers that one's ranges.
 */
export function sharedModel(
	fixtures: readonly PatchFixtureProjection[],
	revisions: readonly PatchProfileRevision[] | null | undefined,
): SharedModel | null {
	const [first] = fixtures;
	if (fixtures.length < 2 || !first) return null;
	if (!fixtures.every((fixture) => fixture.profileId === first.profileId)) return null;
	const known = (revisions ?? []).filter((each) => each.profileId === first.profileId);
	const revision =
		known.find((each) => each.profileRevision === first.profileRevision) ?? known[0] ?? null;
	const snapshot = revision?.profileSnapshot ?? null;
	return {
		profileId: first.profileId,
		label: [revision?.manufacturer, revision?.name].filter(Boolean).join(" ").trim(),
		scenery: snapshot?.scenery ?? null,
		crowd: Boolean(snapshot?.crowd),
	};
}

/**
 * The shared model's own controls, spread over the selection like any other field.
 *
 * A generated object offers the measurements its profile lets the operator set — a stage element's
 * height, a truss's length — each held to that profile's range so a spread cannot push one element
 * out of shape. A placed model offers its scale instead. A lamp has neither.
 */
export function sharedModelFields(model: SharedModel, allVenue: boolean): ThruFieldSpec[] {
	const { scenery } = model;
	if (scenery && hasAdjustableSize(scenery))
		return SIZE_AXES.filter(({ axis }) => scenery.adjustable[axis]).map(
			({ key, label }): ThruFieldSpec => ({
				id: `size-${key}`,
				label: `${label} (m)`,
				ariaLabel: label,
				digits: 3,
				unit: "m",
				read: (fixture) => placedSize(fixture, scenery)[key],
				write: (fixture, metres) => {
					const size = placedSize(fixture, scenery);
					const next = { ...size, [key]: clampToRange(scenery, key, metres ?? size[key]) };
					return {
						...fixture,
						scenerySizeMetres: {
							x: Math.round(next.x * 1000),
							y: Math.round(next.y * 1000),
							z: Math.round(next.z * 1000),
						},
					};
				},
			}),
		);
	if (!allVenue || model.crowd) return [];
	return [
		{
			id: "scale",
			label: "Scale (×)",
			ariaLabel: "Scale",
			digits: 2,
			unit: "×",
			read: (fixture) => fixture.modelScale ?? 1,
			write: (fixture, scale) => ({
				...fixture,
				modelScale: scale == null || scale === 1 ? null : scale,
			}),
		},
	];
}
