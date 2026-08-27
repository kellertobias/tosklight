import type { PatchedFixture } from "../../../api/types";

/**
 * One stretch of a channel that means one thing.
 *
 * A wheel is a run of slots with a rotation band somewhere along it. Turning an encoder over the
 * slots should step from one to the next, but turning it into the rotation band should sweep it,
 * so the two are told apart here rather than at the encoder.
 */
export interface AttributeBand {
	from: number;
	to: number;
	kind: "slot" | "range";
	label: string;
	/// Where a slot is selected from, which is not always the middle of its band.
	rawValue: number;
}

const RAW_MAXIMUM = 255;

function rawOf(normalized: number): number {
	return Math.min(
		RAW_MAXIMUM,
		Math.max(0, Math.round(normalized * RAW_MAXIMUM)),
	);
}

function normalizedOf(raw: number): number {
	return Math.min(1, Math.max(0, raw / RAW_MAXIMUM));
}

/**
 * The bands of one attribute, when every selected fixture lays the channel out the same way.
 *
 * Two fixtures with different gobo wheels have no shared notion of "the next gobo", so a
 * disagreement gives no bands and the encoder stays on plain channel percentage.
 */
export function attributeBands(
	fixtures: readonly PatchedFixture[],
	selectedFixtureIds: readonly string[],
	attribute: string,
): AttributeBand[] | null {
	const selected = new Set(selectedFixtureIds);
	let agreed: AttributeBand[] | undefined;
	for (const fixture of fixtures) {
		if (!selected.has(fixture.fixture_id)) continue;
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		if (!mode) continue;
		for (const channel of mode.channels) {
			if (channel.attribute !== attribute) continue;
			const here = channelBands(channel.functions, attribute);
			if (!here.length) return null;
			if (!agreed) agreed = here;
			else if (!sameBands(agreed, here)) return null;
		}
	}
	return agreed ?? null;
}

/** Only what a band needs, so a channel from any profile shape can be read. */
interface BandFunction {
	dmx_from: number;
	dmx_to: number;
	attribute: string;
	behavior: { type: string; label?: string; raw_value?: number };
}

function channelBands(
	functions: readonly BandFunction[],
	attribute: string,
): AttributeBand[] {
	const bands: AttributeBand[] = [];
	for (const fn of functions) {
		if (fn.attribute !== attribute) continue;
		const behavior = fn.behavior;
		if (behavior.type === "fixed" || behavior.type === "indexed")
			bands.push({
				from: fn.dmx_from,
				to: fn.dmx_to,
				kind: "slot",
				label: behavior.label ?? "",
				rawValue: behavior.raw_value ?? fn.dmx_from,
			});
		else if (behavior.type === "continuous")
			bands.push({
				from: fn.dmx_from,
				to: fn.dmx_to,
				kind: "range",
				label: behavior.label ?? "",
				rawValue: fn.dmx_from,
			});
	}
	return bands.sort((left, right) => left.from - right.from);
}

function sameBands(
	left: readonly AttributeBand[],
	right: readonly AttributeBand[],
): boolean {
	return (
		left.length === right.length &&
		left.every((band, index) => {
			const other = right[index];
			return (
				other !== undefined &&
				band.from === other.from &&
				band.to === other.to &&
				band.kind === other.kind &&
				band.rawValue === other.rawValue
			);
		})
	);
}

/** The band a value sits in, or undefined when it falls in a gap between them. */
export function bandAt(
	bands: readonly AttributeBand[],
	normalized: number,
): AttributeBand | undefined {
	const raw = rawOf(normalized);
	return bands.find((band) => raw >= band.from && raw <= band.to);
}

/** The label a value reads as, when it is sitting on a named band. */
export function bandLabel(
	bands: readonly AttributeBand[],
	normalized: number,
): string | undefined {
	return bandAt(bands, normalized)?.label || undefined;
}

/**
 * Where one detent of the encoder lands.
 *
 * Over slots a detent is one slot, so every position of the encoder is a gobo the fixture can
 * actually reach rather than a raw value between two of them. Inside a ranged area — a wheel
 * spinning, say — a detent is a step in percent of that area, and running off its end steps into
 * whatever is next along the channel.
 */
export function steppedValue(
	bands: readonly AttributeBand[],
	normalized: number,
	direction: 1 | -1,
	coarse: boolean,
): number {
	if (!bands.length) return normalized;
	const raw = rawOf(normalized);
	const current = bandAt(bands, normalized);
	const index = current ? bands.indexOf(current) : -1;
	if (current?.kind === "range") {
		const span = current.to - current.from;
		const stepped = raw + direction * Math.max(1, Math.round(span * (coarse ? 0.1 : 0.02)));
		if (stepped >= current.from && stepped <= current.to)
			return normalizedOf(stepped);
		return entryOf(bands, index + direction, direction) ?? normalizedOf(raw);
	}
	if (index >= 0) return entryOf(bands, index + direction, direction) ?? normalized;
	// A value in a gap steps onto the first band lying that way.
	const reached =
		direction > 0
			? bands.find((band) => band.from > raw)
			: [...bands].reverse().find((band) => band.to < raw);
	return reached ? normalizedOf(enterAt(reached, direction)) : normalized;
}

function entryOf(
	bands: readonly AttributeBand[],
	index: number,
	direction: 1 | -1,
): number | undefined {
	const band = bands[index];
	return band === undefined ? undefined : normalizedOf(enterAt(band, direction));
}

/** A slot is entered where it is selected; a range is entered from the side the encoder came in. */
function enterAt(band: AttributeBand, direction: 1 | -1): number {
	if (band.kind === "slot") return band.rawValue;
	return direction > 0 ? band.from : band.to;
}
