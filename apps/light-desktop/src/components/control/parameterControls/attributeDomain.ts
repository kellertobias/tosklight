import type { PatchedFixture } from "../../../api/types";

/**
 * The scale an attribute is read and typed in.
 *
 * Every programmer value is normalized 0..1 on the wire. What the operator sees and enters is the
 * fixture's own scale: a colour temperature in Kelvin, a blade angle in degrees, a pan as a signed
 * percentage of travel either side of home. The domain is the conversion between the two, and it
 * also carries the step one detent of an encoder should move.
 */
export interface AttributeDomain {
	kind: "percent" | "signed-percent" | "kelvin" | "degrees";
	minimum: number;
	maximum: number;
	suffix: string;
	decimals: number;
	/// One slow detent, in domain units.
	fineStep: number;
	/// One fast detent, in domain units.
	coarseStep: number;
}

const PERCENT: AttributeDomain = {
	kind: "percent",
	minimum: 0,
	maximum: 100,
	suffix: "%",
	decimals: 0,
	fineStep: 0.1,
	coarseStep: 1,
};

/**
 * Pan and tilt read out from home rather than from one end of their travel.
 *
 * Half of the channel is the fixture's home position, so that is where the operator expects to see
 * zero, with the two directions of travel either side of it.
 */
const SIGNED_PERCENT: AttributeDomain = {
	...PERCENT,
	kind: "signed-percent",
	minimum: -100,
	maximum: 100,
	fineStep: 0.2,
	coarseStep: 2,
};

/// The range a colour temperature encoder covers when the fixture does not state its own.
const DEFAULT_TEMPERATURE_RANGE = { minimum: 1_000, maximum: 12_000 };

function positionAttribute(attribute: string): boolean {
	return (
		attribute === "pan" ||
		attribute === "tilt" ||
		attribute.endsWith(".pan") ||
		attribute.endsWith(".tilt")
	);
}

/**
 * The scale one attribute is read in, given what the fixture says about the channel behind it.
 *
 * An angle is only shown in degrees when the profile states the travel it covers; the desk cannot
 * invent a fixture's angular range, so an unstated one stays a percentage of channel travel.
 */
export function attributeDomain(
	attribute: string,
	unit: string | null | undefined,
	physical?: { minimum: number | null; maximum: number | null },
): AttributeDomain {
	if (positionAttribute(attribute)) return SIGNED_PERCENT;
	const stated =
		physical &&
		physical.minimum !== null &&
		physical.maximum !== null &&
		physical.minimum !== physical.maximum
			? { minimum: physical.minimum, maximum: physical.maximum }
			: null;
	if (unit === "K") {
		const range = stated ?? DEFAULT_TEMPERATURE_RANGE;
		return {
			kind: "kelvin",
			...range,
			suffix: " K",
			decimals: 0,
			fineStep: 10,
			coarseStep: 100,
		};
	}
	if (unit === "deg" && stated) {
		return {
			kind: "degrees",
			...stated,
			suffix: "°",
			decimals: 1,
			fineStep: 0.5,
			coarseStep: 5,
		};
	}
	return PERCENT;
}

/** The value the operator reads, from the value on the wire. */
export function domainValue(domain: AttributeDomain, normalized: number): number {
	const span = domain.maximum - domain.minimum;
	return domain.minimum + Math.min(1, Math.max(0, normalized)) * span;
}

/** The value on the wire, from the value the operator typed. */
export function normalizedValue(domain: AttributeDomain, value: number): number {
	const span = domain.maximum - domain.minimum;
	if (!span) return 0;
	return Math.min(1, Math.max(0, (value - domain.minimum) / span));
}

export function formatAttributeValue(
	domain: AttributeDomain,
	normalized: number,
): string {
	const value = domainValue(domain, normalized);
	const rounded = value.toFixed(domain.decimals);
	// Rounding a hair below home would otherwise read as "-0%".
	const shown = Number(rounded) === 0 ? (0).toFixed(domain.decimals) : rounded;
	return `${shown}${domain.suffix}`;
}

/** One detent, expressed as the normalized delta the desk is asked for. */
export function domainStep(domain: AttributeDomain, coarse: boolean): number {
	const span = domain.maximum - domain.minimum;
	if (!span) return 0;
	return (coarse ? domain.coarseStep : domain.fineStep) / span;
}

/**
 * What one channel of one attribute says about its own scale, when every selected fixture agrees.
 *
 * Fixtures of different types can be selected together, and a Kelvin range that is right for one of
 * them is wrong for another, so a disagreement falls back to plain channel percentage.
 */
export function selectedChannelUnit(
	fixtures: readonly PatchedFixture[],
	selectedFixtureIds: readonly string[],
	attribute: string,
): { unit: string | null; minimum: number | null; maximum: number | null } | null {
	const selected = new Set(selectedFixtureIds);
	let agreed:
		| { unit: string | null; minimum: number | null; maximum: number | null }
		| undefined;
	for (const fixture of fixtures) {
		if (!selected.has(fixture.fixture_id)) continue;
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		if (!mode) continue;
		for (const channel of mode.channels) {
			if (channel.attribute !== attribute) continue;
			const here = {
				unit: channel.unit,
				minimum: channel.physical_min,
				maximum: channel.physical_max,
			};
			if (!agreed) agreed = here;
			else if (
				agreed.unit !== here.unit ||
				agreed.minimum !== here.minimum ||
				agreed.maximum !== here.maximum
			)
				return null;
		}
	}
	return agreed ?? null;
}
