import type { PatchedFixture } from "../../../api/types";
import { parsePatchAddress } from "../../input/ConsoleFields";
import { conflicts } from "../patchUtils";

export function fixtureDisplayId(
	fixture: Pick<PatchedFixture, "fixture_number" | "virtual_fixture_number">,
) {
	return fixture.virtual_fixture_number != null
		? `0.${fixture.virtual_fixture_number}`
		: (fixture.fixture_number ?? "—");
}

export function compareFixtureIds(a: PatchedFixture, b: PatchedFixture) {
	if (
		a.virtual_fixture_number != null &&
		b.virtual_fixture_number != null &&
		a.virtual_fixture_number !== b.virtual_fixture_number
	)
		return a.virtual_fixture_number - b.virtual_fixture_number;
	if (a.virtual_fixture_number != null) return -1;
	if (b.virtual_fixture_number != null) return 1;
	if (
		a.fixture_number != null &&
		b.fixture_number != null &&
		a.fixture_number !== b.fixture_number
	)
		return a.fixture_number - b.fixture_number;
	if (a.fixture_number != null) return -1;
	if (b.fixture_number != null) return 1;
	return a.fixture_id.localeCompare(b.fixture_id);
}

const MAX_FIXTURE_NUMBER = 4_294_967_295;

export function parseFixtureNumber(value: string): number | null {
	const number = Number(value);
	return Number.isInteger(number) && number >= 1 && number <= MAX_FIXTURE_NUMBER
		? number
		: null;
}

export function parseVirtualFixtureNumber(value: string): number | null {
	const match = /^0\.(\d+)$/.exec(value.trim());
	if (!match) return null;
	const number = Number(match[1]);
	return Number.isInteger(number) && number >= 1 && number <= MAX_FIXTURE_NUMBER
		? number
		: null;
}

export function nextAvailableFixtureNumber(
	start: number,
	used: ReadonlySet<number>,
): number | null {
	let number = start;
	while (number <= MAX_FIXTURE_NUMBER && used.has(number)) number++;
	return number <= MAX_FIXTURE_NUMBER ? number : null;
}

export function placementBatchCount(value: string) {
	return Math.max(1, Math.floor(Number(value) || 1));
}

export function contiguousBatchPatches(
	universe: number,
	address: number,
	count: number,
	footprint: number,
) {
	return Array.from(
		{ length: count },
		(_, index) => `${universe}.${address + index * footprint}`,
	);
}

export function resizeBatchPatches(
	current: string[],
	count: number,
	universe: number,
	address: number,
	footprint: number,
) {
	if (current.length >= count) return current.slice(0, count);
	const next = [...current];
	while (next.length < count) {
		const previous = parsePatchAddress(next.at(-1) ?? "") ?? {
			universe,
			address: address - footprint,
		};
		next.push(`${previous.universe}.${previous.address + footprint}`);
	}
	return next;
}

export function batchPatchError(
	patches: Array<{ universe: number; address: number } | null>,
	footprint: number,
	fixtures: PatchedFixture[],
) {
	const validPatches = patches.filter(
		(patch): patch is { universe: number; address: number } => patch != null,
	);
	if (!patches.length || validPatches.length !== patches.length)
		return "Choose a valid DMX address for every fixture in the batch.";
	const ranges = validPatches.map((patch, index) => ({
		index,
		universe: patch.universe,
		start: patch.address,
		end: patch.address + footprint - 1,
	}));
	if (ranges.some((range) => range.start < 1 || range.end > 512))
		return "Every fixture in the batch must fit completely inside one 512-slot universe.";
	if (
		ranges.some(
			(range) =>
				conflicts(fixtures, range.universe, range.start, footprint).length,
		)
	)
		return "One or more fixture patches overlap an occupied DMX range.";
	for (let index = 0; index < ranges.length; index++)
		for (let other = index + 1; other < ranges.length; other++) {
			const left = ranges[index];
			const right = ranges[other];
			if (
				left.universe === right.universe &&
				left.start <= right.end &&
				right.start <= left.end
			)
				return `Fixture ${left.index + 1} overlaps fixture ${right.index + 1} in this batch.`;
		}
	return null;
}
