import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useFixtureSheetRows } from "./fixtureSheetProjection";

vi.mock("../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	usePatchedFixturesView: (enabled = true) =>
		enabled ? mocks.server.patch.fixtures : [],
}));

const mocks = vi.hoisted(() => {
	const runtime = {
		master: 0.4,
		flashLevel: 0,
		playbackNumber: 17 as number | null,
	};
	const group = {
		kind: "group" as const,
		id: "front",
		revision: 1,
		updated_at: "",
		body: {
			name: "Front",
			fixtures: ["fixture-1"],
			master: 1,
			playback_fader: null,
			programming: {},
		},
		runtime,
	};
	const fixture = {
		fixture_id: "fixture-1",
		fixture_number: 1,
		name: "Fixture 1",
		universe: 1,
		address: 1,
		definition: {
			schema_version: 1,
			id: "definition",
			revision: 1,
			manufacturer: "Test",
			device_type: "fixture",
			name: "Fixture",
			model: "Fixture",
			mode: "1ch",
			footprint: 1,
			heads: [
				{
					index: 0,
					name: "Base",
					shared: true,
					parameters: [
						{
							attribute: "intensity",
							components: [],
							default: 0,
							virtual_dimmer: false,
							capabilities: [],
						},
					],
				},
			],
			color_calibration: null,
			physical: {},
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" as const },
			safe_values: {},
		},
		logical_heads: [],
	};
	const server = {
		bootstrap: { active_show: { id: "show-a" } },
		patch: { fixtures: [fixture], revision: 1 },
	};
	Object.defineProperty(server, "playbacks", {
		get() {
			throw new Error("Fixture Sheet must not read broad playbacks");
		},
	});
	return {
		group,
		groups: [group],
		ready: true,
		runtime,
		server,
	};
});

vi.mock("../api/ServerContext", () => ({ useServer: () => mocks.server }));
vi.mock("../features/groupRuntime/groupRuntimeAuthority", () => ({
	useGroupRuntimeAuthority: () => ({
		ready: mocks.ready,
		loading: !mocks.ready,
		canWrite: true,
		groups: mocks.groups,
		setMaster: vi.fn(),
		setFlash: vi.fn(),
	}),
}));
vi.mock("../features/programmerValues/useProgrammerValueTargets", () => ({
	useProgrammerValueTargets: () => [],
}));

function rows() {
	return useFixtureSheetRows({
		visualization: {
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [],
		},
		preloadVisualization: null,
		fixtureOrder: "fixture-id",
		activeOnly: false,
		selectedCueList: null,
		includedHeads: "all",
		active: true,
	});
}

beforeEach(() => {
	mocks.ready = true;
	mocks.runtime.master = 0.4;
	mocks.runtime.playbackNumber = 17;
});

afterEach(cleanup);

describe("Fixture Sheet scoped Group runtime", () => {
	it("uses authoritative assignment and master for limiting-group output", () => {
		const view = renderHook(rows);

		expect(view.result.current.rows).toHaveLength(1);
		expect(view.result.current.rows[0].limitingGroups).toEqual([mocks.group]);
		expect(mocks.group.body.playback_fader).toBeNull();
		expect(mocks.group.body.master).toBe(1);

		mocks.runtime.playbackNumber = null;
		view.rerender();
		expect(view.result.current.rows[0].limitingGroups).toEqual([]);
	});

	it("exposes no rows while exact Group runtime authority is loading", () => {
		mocks.ready = false;
		const view = renderHook(rows);

		expect(view.result.current.rows).toEqual([]);
		expect(view.result.current.groupRuntimeLoading).toBe(true);
	});
});
