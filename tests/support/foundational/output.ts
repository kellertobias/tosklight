import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";

export async function expectSlotsAfterTick(
	bench: any,
	millis: number,
	expected: number[],
): Promise<void> {
	const artnetMark = bench.artnet.mark();
	const sacnMark = bench.sacn.mark();
	const tick = await bench.tick(millis);
	const slots =
		tick.universes.find((universe: any) => universe.universe === 1)?.slots ??
		[];
	expect(slots.slice(0, expected.length)).toEqual(expected);
	const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
	const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
	expect(Array.from(artnet.slots.slice(0, expected.length))).toEqual(expected);
	expect(Array.from(sacn.slots.slice(0, expected.length))).toEqual(expected);
}

export function slotsFromFrame(
	frame: { universes: Array<{ universe: number; slots: number[] }> },
	count: number,
): number[] {
	return (
		frame.universes.find((universe) => universe.universe === 1)?.slots ?? []
	).slice(0, count);
}

export function normalized(
	value: { value?: number } | number | undefined,
): number | undefined {
	return typeof value === "number" ? value : value?.value;
}
