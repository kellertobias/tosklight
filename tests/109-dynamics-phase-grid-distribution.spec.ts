import { expect, test } from "./bench/core/fixtures";

interface StoredStageLayoutBody {
	version?: number;
	positions?: Record<string, { x: number; y: number }>;
	positions3d?: Record<string, { x: number; y: number; z: number }>;
}

interface GroupSettingsSnapshot {
	resolved_spatial: {
		ranks: Array<{ fixture_id: string; rank: number }>;
		rank_count: number;
	} | null;
}

/** A grid `side` fixtures across, one metre apart, laid out in the Top projection's own plane. */
function gridPositions(fixtureIds: readonly string[], side: number) {
	return Object.fromEntries(
		fixtureIds.slice(0, side * side).map((fixtureId, index) => [
			fixtureId,
			{ x: (index % side) * 1_000, y: Math.floor(index / side) * 1_000, z: 0 },
		]),
	);
}

function mappingAt(angleDegrees: number) {
	return {
		projection: {
			anchor: { x: 0, y: 0, z: 0 },
			view_direction: { x: 0, y: 0, z: -1 },
			rotation_degrees: 0,
			preset: "top",
		},
		shape: { type: "grid", angle_degrees: angleDegrees, direction: "ascending" },
	};
}

/**
 * The Phase Grid distribution, resolved by the desk that a show is loaded into.
 *
 * A grid ranks by the lines its direction makes, so every fixture standing on one of those lines
 * shares a rank and the rank advances only from line to line. This covers what an automated bench
 * can: the distribution itself and that it survives a reload. Whether it reads the same on an
 * attached control surface still needs a human at the desk — see TL-462.
 */
test.describe("docs/testing: Dynamics Phase Grid distribution", () => {
	test("PHASE-GRID-001 @api › a grid gives every fixture on one line the same rank, and keeps it across a restart", async ({
		api,
		bench,
		show,
	}) => {
		test.setTimeout(120_000);
		// The bench show has twelve fixtures, so three by three is the largest square it holds.
		const side = 3;
		const members = show.fixtureIds.slice(0, side * side);
		expect(members.length).toBe(side * side);

		const stage = await api.showObject<StoredStageLayoutBody>(
			show.id,
			"stage_layout",
			"main",
		);
		await api.seedShowObject(
			show.id,
			"stage_layout",
			"main",
			{
				version: stage?.body.version ?? 2,
				positions: stage?.body.positions ?? {},
				positions3d: gridPositions(members, side),
			},
			stage?.revision ?? 0,
		);

		/** Seeds the Group at `angle` and returns each member's rank, by grid index. */
		const ranksAt = async (angle: number) => {
			const existing = await api.showObject(show.id, "group", "7");
			await api.seedShowObject(
				show.id,
				"group",
				"7",
				{
					id: "7",
					name: "Phase grid",
					fixtures: members,
					source: { type: "explicit", fixture_ids: members },
					mapping: mappingAt(angle),
					derived_from: null,
					frozen_from: null,
					programming: {},
				},
				existing?.revision ?? 0,
			);
			const snapshot = await api.request<GroupSettingsSnapshot>(
				"GET",
				"/api/v2/groups/7",
			);
			const spatial = snapshot.resolved_spatial;
			if (!spatial) throw new Error("the Group resolved no spatial ranking");
			const byFixture = new Map(
				spatial.ranks.map((entry) => [entry.fixture_id, entry.rank]),
			);
			return {
				rankCount: spatial.rank_count,
				ranks: members.map((fixtureId) => {
					const rank = byFixture.get(fixtureId);
					if (rank === undefined)
						throw new Error(`fixture ${fixtureId} has no rank`);
					return rank;
				}),
			};
		};

		// 90 degrees runs row by row: a row shares one rank, and the rank steps per row.
		const rows = await ranksAt(90);
		expect(rows.rankCount).toBe(side);
		expect(rows.ranks).toEqual(
			members.map((_, index) => Math.floor(index / side)),
		);

		// 0 degrees runs column by column instead.
		const columns = await ranksAt(0);
		expect(columns.rankCount).toBe(side);
		expect(columns.ranks).toEqual(members.map((_, index) => index % side));

		// 45 degrees runs corner to corner, each anti-diagonal moving as one.
		const diagonals = await ranksAt(45);
		expect(diagonals.rankCount).toBe(side * 2 - 1);
		expect(diagonals.ranks).toEqual(
			members.map((_, index) => Math.floor(index / side) + (index % side)),
		);

		// The configuration and its distribution survive the desk being restarted.
		await bench.restart();
		// A restarted desk issues new sessions, so the bench signs in again before reading.
		await api.login();
		const reloaded = await api.request<GroupSettingsSnapshot>(
			"GET",
			"/api/v2/groups/7",
		);
		expect(reloaded.resolved_spatial?.rank_count).toBe(side * 2 - 1);
		const byFixture = new Map(
			(reloaded.resolved_spatial?.ranks ?? []).map((entry) => [
				entry.fixture_id,
				entry.rank,
			]),
		);
		expect(members.map((fixtureId) => byFixture.get(fixtureId))).toEqual(
			diagonals.ranks,
		);
	});
});
