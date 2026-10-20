import { cleanup, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { decodeVisualizationRuntimeSnapshot } from "../api/VisualizationRuntimeTransport";
import { srgbToXyz } from "../features/presetPreview/colorDisplay";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
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
		playbackAddress: { page: 1, position: 17 } as {
			page: number;
			position: number;
		} | null,
	};
	const group = {
		kind: "group" as const,
		id: "front",
		revision: 1,
		updated_at: "",
		body: {
			name: "Front",
			fixtures: ["fixture-1"],
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
		group_masters_enabled: true,
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
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapReady: () => mocks.server.bootstrap !== null,
	useActiveShowId: () => mocks.server.bootstrap?.active_show?.id ?? null,
	useAttributeRegistry: () => [
		{
			id: "intensity",
			label: "Intensity",
			family: "intensity",
			value_type: "continuous",
			default_unit: "percent",
			encoder_group: "intensity",
			encoder_page: 1,
			encoder_slot: 1,
		},
	],
}));
vi.mock("../features/groupRuntime/groupRuntimeAuthority", () => ({
	useGroupRuntimeAuthority: () => ({
		ready: mocks.ready,
		serving: mocks.ready,
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

function rows(
	highlight: Parameters<typeof useFixtureSheetRows>[0]["highlight"] = null,
) {
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
		highlight,
		active: true,
	});
}

beforeEach(() => {
	mocks.ready = true;
	mocks.runtime.master = 0.4;
	mocks.runtime.flashLevel = 0;
	mocks.runtime.playbackAddress = { page: 1, position: 17 };
	mocks.server.patch.fixtures[0].group_masters_enabled = true;
});

afterEach(cleanup);

describe("Fixture Sheet scoped Group runtime", () => {
	it("carries decoded wire Preset provenance through the real sheet rows and Color cell, then clears it for identical literals", () => {
		const wire = {
			scope: { show_id: "show-a" }, revision: 1, generated_at: "2026-10-20T12:00:00.000Z",
			grand_master: 1, blackout: false, preload: false,
			values: [{ fixture_id: "fixture-1", attribute: "color", value: { kind: "color_xyz", value: srgbToXyz(1, 0, 0) } }],
			profile_output_values: [],
			color_preset_names: [{ fixture_id: "fixture-1", preset_instance_id: "33333333-3333-4333-8333-333333333333", name: "Opening ruby" }],
		};
		let visualization = decodeVisualizationRuntimeSnapshot(wire, "normal");
		const view = renderHook(() => useFixtureSheetRows({
			visualization, preloadVisualization: null, fixtureOrder: "fixture-id", activeOnly: false,
			selectedCueList: null, includedHeads: "all", highlight: null, active: true,
		}));
		expect(view.result.current.rows[0].colorLabel).toBe("Opening ruby");
		const column = fixtureSheetColumns(false, () => ({ base: false, containedBase: false, containedCurrent: false, current: false })).find(column => column.id === "color")!;
		const cell = render(<>{column.render(view.result.current.rows[0], 0)}</>);
		expect(screen.getByText("Opening ruby")).toBeVisible();
		expect(screen.queryByText("<Red>")).toBeNull();
		visualization = decodeVisualizationRuntimeSnapshot({ ...wire, color_preset_names: [] }, "normal");
		view.rerender();
		expect(view.result.current.rows[0].colorLabel).toBe("<Red>");
		cell.rerender(<>{column.render(view.result.current.rows[0], 0)}</>);
		expect(screen.getByText("<Red>")).toBeVisible();
		expect(screen.queryByText("Opening ruby")).toBeNull();
	});

	it("uses authoritative assignment and master for limiting-group output", () => {
		const view = renderHook(rows);

		expect(view.result.current.rows).toHaveLength(1);
		expect(view.result.current.rows[0].limitingGroups).toEqual([mocks.group]);
		expect(view.result.current.rows[0].limitingGroups[0].runtime.master).toBe(
			0.4,
		);
		const nameColumn = fixtureSheetColumns(false, () => ({
			base: false,
			containedBase: false,
			containedCurrent: false,
			current: false,
		})).find(({ id }) => id === "name");
		render(nameColumn?.render(view.result.current.rows[0], 0) ?? null);
		expect(screen.getByText("◒ Group master 40%")).toHaveAttribute(
			"title",
			"Front: 40%",
		);

		mocks.runtime.playbackAddress = null;
		view.rerender();
		expect(view.result.current.rows[0].limitingGroups).toEqual([]);
	});

	it("shows effective Flash level and active Highlight bypass without losing Group status", () => {
		mocks.runtime.flashLevel = 0.8;
		const highlight = {
			active: true,
			mode: "selection" as const,
			output_enabled: true,
			capture_only: false,
			remembered: [{ fixture_id: "fixture-1" }],
			active_index: null,
			active_fixture: null,
			can_previous: false,
			can_next: false,
			owner_user_id: "operator-a",
		};
		const view = renderHook(() => rows(highlight));
		expect(view.result.current.rows[0].highlightBypassesGroupMaster).toBe(true);
		const nameColumn = fixtureSheetColumns(false, () => ({
			base: false,
			containedBase: false,
			containedCurrent: false,
			current: false,
		})).find(({ id }) => id === "name");
		render(nameColumn?.render(view.result.current.rows[0], 0) ?? null);
		expect(
			screen.getByText("◒ Group master bypassed · Highlight"),
		).toHaveAttribute("data-group-master-state", "highlight-bypass");
		expect(
			screen.getByText("◒ Group master bypassed · Highlight"),
		).toHaveAttribute(
			"title",
			"Front: fader 40%, Flash 80%, effective 80%; bypassed by Highlight",
		);

		cleanup();
		render(
			nameColumn?.render(
				{ ...view.result.current.rows[0], highlightBypassesGroupMaster: false },
				0,
			) ?? null,
		);
		expect(screen.getByText("◒ Group master 80% · Flash")).toHaveAttribute(
			"data-group-master-state",
			"flash",
		);
	});

	it("does not report or present a full Group Master while Flash is pressed", () => {
		mocks.runtime.master = 1;
		const view = renderHook(rows);
		expect(view.result.current.rows[0].limitingGroups).toEqual([]);

		mocks.runtime.flashLevel = 1;
		view.rerender();
		expect(view.result.current.rows[0].limitingGroups).toEqual([]);

		const nameColumn = fixtureSheetColumns(false, () => ({
			base: false,
			containedBase: false,
			containedCurrent: false,
			current: false,
		})).find(({ id }) => id === "name");
		render(nameColumn?.render(view.result.current.rows[0], 0) ?? null);
		expect(screen.queryByText(/Group master/i)).toBeNull();

		mocks.runtime.master = 0.6;
		view.rerender();
		expect(view.result.current.rows[0].limitingGroups).toEqual([mocks.group]);
	});

	it("exposes no rows while exact Group runtime authority is loading", () => {
		mocks.ready = false;
		const view = renderHook(rows);

		expect(view.result.current.rows).toEqual([]);
		expect(view.result.current.groupRuntimeLoading).toBe(true);
	});

	it("does not report a limiting Group when the logical fixture ignores Group Masters", () => {
		mocks.server.patch.fixtures[0].group_masters_enabled = false;
		const view = renderHook(rows);
		expect(view.result.current.rows[0].limitingGroups).toEqual([]);
	});
});
