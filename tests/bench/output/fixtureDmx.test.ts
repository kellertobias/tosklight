import { describe, expect, it, vi } from "vitest";
import type {
	DmxSnapshot,
	PatchedFixture,
	PatchSnapshot,
} from "../../../apps/light-desktop/src/api/types";
import { FixtureDmxAssertions, fixture, fixtureRange } from "./fixtureDmx";

describe("fixture-aware logical DMX", () => {
	it("resolves coarse, fine, and ultra bytes from the current sparse profile layout", async () => {
		const fixture101 = profileFixture();
		const frame = logicalFrame([
			[101, 127],
			[100, 5],
			[103, 31],
		]);
		const assertions = new FixtureDmxAssertions(source([fixture101], frame), 0);

		await assertions.expect(fixture(101), {
			"Pan coarse": 127,
			"pan fine": { between: [0, 16] },
			" Pan ultra ": { between: [31, 31] },
		});
	});

	it("follows repatching and supports explicit multi-patch ownership", async () => {
		const fixture101 = profileFixture();
		const state = source([fixture101], logicalFrame([[101, 80]]));
		const assertions = new FixtureDmxAssertions(state, 0);
		await assertions.expect(fixture(101), { "Pan coarse": 80 });

		const primary = fixture101.split_patches?.[0];
		if (!primary) throw new Error("Test fixture has no primary split");
		primary.address = 300;
		state.frame = logicalFrame([[301, 90]]);
		await assertions.expect(fixture(101), { "Pan coarse": 90 });

		state.frame = logicalFrame([[201, 100]]);
		await assertions.expect(fixture(101, { multipatch: "Mirror" }), {
			"Pan coarse": 100,
		});
	});

	it("applies one expectation to every fixture in a typed range", async () => {
		const first = profileFixture();
		const second = {
			...profileFixture(),
			fixture_id: "fixture-102",
			fixture_number: 102,
			split_patches: [{ split: 1, universe: 1, address: 400 }],
		};
		const assertions = new FixtureDmxAssertions(
			source(
				[first, second],
				logicalFrame([
					[101, 127],
					[401, 127],
				]),
			),
			0,
		);
		await assertions.expect(fixtureRange(101, 102), { "Pan coarse": 127 });
	});

	it("rejects invalid values and diagnoses an unpatched channel", async () => {
		const unpatched = profileFixture();
		unpatched.split_patches = [{ split: 1, universe: null, address: null }];
		const assertions = new FixtureDmxAssertions(
			source([unpatched], logicalFrame([])),
			0,
		);
		await expect(
			assertions.expect(fixture(101), { "Pan coarse": 256 }),
		).rejects.toThrow("integer from 0 through 255");
		await expect(
			assertions.expect(fixture(101), { "Pan coarse": 0 }),
		).rejects.toThrow(
			"primary fixture patch has no DMX assignment for Pan coarse on split 1",
		);
		await assertions.expectAbsent(fixture(101));
	});
});

function source(fixtures: PatchedFixture[], initialFrame: DmxSnapshot) {
	const state = {
		frame: initialFrame,
		patch: vi.fn(async () => ({
			revision: 7,
			fixtures,
			routes: [],
		})),
		request: vi.fn(async <T>() => state.frame as T),
	} satisfies {
		frame: DmxSnapshot;
		patch: () => Promise<PatchSnapshot>;
		request: <T>() => Promise<T>;
	};
	return state;
}

function logicalFrame(values: Array<[number, number]>): DmxSnapshot {
	const slots = Array(512).fill(0);
	for (const [address, value] of values) slots[address - 1] = value;
	return { revision: 11, universes: [{ universe: 1, slots }], overrides: [] };
}

function profileFixture(): PatchedFixture {
	const profile = {
		schema_version: 2,
		id: "profile",
		revision: 4,
		manufacturer: "Test",
		name: "Mover",
		short_name: "Mover",
		fixture_type: "moving light",
		modes: [
			{
				id: "mode",
				name: "24-bit",
				splits: [{ number: 1, footprint: 4 }],
				heads: [{ id: "head", name: "Main", master_shared: true }],
				channels: [
					{
						id: "pan",
						head_id: "head",
						split: 1,
						attribute: "pan",
						resolution: "u24",
						secondary_slots: [1, 4],
					},
				],
			},
		],
	};
	return {
		fixture_id: "fixture-101",
		fixture_number: 101,
		name: "Mover 101",
		universe: 1,
		address: 100,
		split_patches: [{ split: 1, universe: 1, address: 100 }],
		layer_id: "default",
		definition: {
			schema_version: 2,
			revision: 4,
			name: "Mover",
			mode: "24-bit",
			mode_id: "mode",
			profile_snapshot: profile,
		},
		logical_heads: [],
		multipatch: [
			{
				id: "mirror",
				name: "Mirror",
				universe: 1,
				address: 200,
				split_patches: [{ split: 1, universe: 1, address: 200 }],
				location: { x: 0, y: 0, z: 0 },
				rotation: { x: 0, y: 0, z: 0 },
			},
		],
	} as unknown as PatchedFixture;
}
