import type { DmxSnapshot } from "../../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../core/api";

type ChannelMap = Readonly<Record<number, number>>;

interface DmxSource {
	request<T>(
		method: string,
		path: string,
		body?: unknown,
		authenticate?: boolean,
	): Promise<T>;
}

export class BrowserDmx {
	constructor(private readonly source: DmxSource) {}

	/** Returns the latest logical frame without advancing application time. */
	async frame(universe: number): Promise<readonly number[]> {
		assertUniverse(universe);
		const snapshot = await this.snapshot();
		return (
			snapshot.universes.find((candidate) => candidate.universe === universe)
				?.slots ?? []
		);
	}

	expect(universe: number): DmxUniverseExpectation {
		assertUniverse(universe);
		return new DmxUniverseExpectation(this, universe);
	}

	async waitFor(
		universe: number,
		channels: ChannelMap,
		timeout = 2_000,
	): Promise<void> {
		assertUniverse(universe);
		const expected = validateChannels(channels);
		const deadline = Date.now() + timeout;
		let last: DmxSnapshot | undefined;
		do {
			last = await this.snapshot();
			const frame = last.universes.find(
				(candidate) => candidate.universe === universe,
			)?.slots;
			if (
				frame &&
				expected.every(([address, value]) => frame[address - 1] === value)
			)
				return;
			await new Promise((resolve) => setTimeout(resolve, 10));
		} while (Date.now() < deadline);
		throw new Error(
			`Logical DMX U${universe} did not reach ${expected.map(([address, value]) => `${address}=${value}`).join(", ")}; latest frame revision ${last?.revision ?? "(unavailable)"}: ${JSON.stringify(last?.universes.find((candidate) => candidate.universe === universe)?.slots ?? [])}`,
		);
	}

	private snapshot(): Promise<DmxSnapshot> {
		return this.source.request("GET", "/api/v2/output/dmx", undefined, false);
	}
}

export class DmxUniverseExpectation {
	constructor(
		private readonly dmx: BrowserDmx,
		private readonly universe: number,
	) {}

	channel(address: number, value: number): Promise<void> {
		return this.dmx.waitFor(this.universe, { [address]: value });
	}

	channels(values: ChannelMap): Promise<void> {
		return this.dmx.waitFor(this.universe, values);
	}

	range(start: number, values: readonly number[]): Promise<void> {
		assertAddress(start);
		if (values.length === 0)
			throw new Error("Logical DMX range must contain at least one byte");
		if (start + values.length - 1 > 512)
			throw new Error("Logical DMX range exceeds address 512");
		return this.channels(
			Object.fromEntries(values.map((value, index) => [start + index, value])),
		);
	}
}

export function browserDmx(api: ApiDriver): BrowserDmx {
	return new BrowserDmx(api);
}

function validateChannels(channels: ChannelMap): Array<[number, number]> {
	const entries = Object.entries(channels).map(
		([address, value]) => [Number(address), value] as [number, number],
	);
	if (entries.length === 0)
		throw new Error(
			"Logical DMX expectation must contain at least one channel",
		);
	for (const [address, value] of entries) {
		assertAddress(address);
		assertByte(value);
	}
	return entries;
}

function assertUniverse(universe: number): void {
	if (!Number.isInteger(universe) || universe < 1)
		throw new Error("DMX universe must be a positive integer");
}

function assertAddress(address: number): void {
	if (!Number.isInteger(address) || address < 1 || address > 512)
		throw new Error("DMX address must be an integer from 1 through 512");
}

function assertByte(value: number): void {
	if (!Number.isInteger(value) || value < 0 || value > 255)
		throw new Error("DMX byte must be an integer from 0 through 255");
}
