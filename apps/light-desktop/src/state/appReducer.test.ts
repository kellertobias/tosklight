import { describe, expect, it } from "vitest";
import { createVisualizationWidget } from "../windows/visualizationPaneModel";
import { appReducer, initialState } from "./appReducer";

describe("appReducer control mode and pane geometry", () => {
	it("keeps programmer and playback as explicit switchable control modes", () => {
		const playback = appReducer(initialState, { type: "TOGGLE_CONTROL_MODE" });
		expect(playback.controlMode).toBe("playbacks");
		expect(
			appReducer(playback, { type: "TOGGLE_CONTROL_MODE" }).controlMode,
		).toBe("programmer");
	});

	it("keeps resized panes inside the 24 by 18 grid", () => {
		const isolated = {
			...initialState,
			desks: initialState.desks.map((desk, index) =>
				index ? desk : { ...desk, panes: desk.panes.slice(0, 1) },
			),
		};
		const changed = appReducer(isolated, {
			type: "SET_PANE_RECT",
			id: isolated.desks[0].panes[0].id,
			rect: { x: 23, y: 17, width: 12, height: 12 },
		});
		expect(changed.desks[0].panes[0]).toMatchObject({
			x: 23,
			y: 17,
			width: 2,
			height: 2,
		});
	});

	it("rejects pane moves and resizes that overlap another pane", () => {
		const pane = initialState.desks[0].panes[0];
		const blocker = initialState.desks[0].panes[1];
		const changed = appReducer(initialState, {
			type: "SET_PANE_RECT",
			id: pane.id,
			rect: {
				x: blocker.x,
				y: blocker.y,
				width: blocker.width,
				height: blocker.height,
			},
		});
		expect(changed.desks[0].panes[0]).toEqual(pane);
	});
});

describe("appReducer desk creation and layout hydration", () => {
	it("creates an empty new desk normally and clones when saving as new", () => {
		const empty = appReducer(initialState, { type: "NEW_DESK" });
		expect(empty.desks.at(-1)?.panes).toHaveLength(0);
		expect(empty.desks.at(-1)?.name).toBe("Desktop 4");

		const saving = appReducer(initialState, { type: "START_SAVE_DESK" });
		const cloned = appReducer(saving, { type: "NEW_DESK" });
		expect(cloned.desks.at(-1)?.panes).toHaveLength(
			initialState.desks[0].panes.length,
		);
		expect(cloned.desks.at(-1)?.panes[0].id).not.toBe(
			initialState.desks[0].panes[0].id,
		);
	});

	it("allocates a unique Desktop id after a sparse deletion", () => {
		const sparse = {
			...initialState,
			desks: [
				initialState.desks[0],
				{ ...initialState.desks[1], id: "desk-4" },
				{ ...initialState.desks[2], id: "desk-5" },
			],
		};
		const created = appReducer(sparse, { type: "NEW_DESK" });
		expect(created.desks.at(-1)?.id).toBe("desk-6");
	});

	it("copies the active desk into an existing save target and hydrates stored layouts", () => {
		const saving = appReducer(initialState, { type: "START_SAVE_DESK" });
		const saved = appReducer(saving, { type: "SAVE_DESK_TO", id: "playback" });
		expect(saved.activeDeskId).toBe("playback");
		expect(
			saved.desks.find((desk) => desk.id === "playback")?.panes,
		).toHaveLength(initialState.desks[0].panes.length);

		const hydrated = appReducer(saved, {
			type: "HYDRATE_LAYOUT",
			desks: [{ id: "tour", name: "Tour", panes: [] }],
			activeDeskId: "tour",
		});
		expect(hydrated.desks).toEqual([{ id: "tour", name: "Tour", panes: [] }]);
		expect(hydrated.activeDeskId).toBe("tour");
	});

	it("does not override operator navigation when layout hydration finishes late", () => {
		const navigating = appReducer(initialState, {
			type: "OPEN_BUILTIN",
			kind: "groups",
		});
		const hydrated = appReducer(navigating, {
			type: "HYDRATE_LAYOUT",
			desks: [{ id: "tour", name: "Tour", panes: [] }],
			activeDeskId: "tour",
		});
		expect(hydrated.builtIn).toBe("groups");
		expect(hydrated.dockMode).toBe("builtins");
	});

	it("creates and safely hydrates persisted Scheduler pane layout", () => {
		const emptyDesk = {
			...initialState,
			activeDeskId: "scheduler-test",
			desks: [{ id: "scheduler-test", name: "Scheduler", panes: [] }],
			windowPicker: { x: 1, y: 1, width: 12, height: 10 },
		};
		const added = appReducer(emptyDesk, {
			type: "ADD_WINDOW",
			kind: "scheduler",
		});
		expect(added.desks[0].panes[0]).toMatchObject({
			kind: "scheduler",
			title: "Scheduler",
			schedulerShowList: true,
			schedulerShowCalendar: true,
		});

		const hydrated = appReducer(added, {
			type: "HYDRATE_LAYOUT",
			desks: [
				{
					id: "scheduler-test",
					name: "Scheduler",
					panes: [
						{
							...added.desks[0].panes[0],
							schedulerShowList: false,
							schedulerShowCalendar: false,
						},
					],
				},
			],
			activeDeskId: "scheduler-test",
		});
		expect(hydrated.desks[0].panes[0]).toMatchObject({
			schedulerShowList: true,
			schedulerShowCalendar: false,
		});
	});

	it("creates, edits, and safely hydrates show-persisted Visualization rows", () => {
		const emptyDesk = {
			...initialState,
			activeDeskId: "visualization-test",
			desks: [{ id: "visualization-test", name: "Visualization", panes: [] }],
			windowPicker: { x: 1, y: 1, width: 12, height: 10 },
		};
		const added = appReducer(emptyDesk, {
			type: "ADD_WINDOW",
			kind: "visualization",
		});
		const pane = added.desks[0].panes[0];
		expect(pane).toMatchObject({
			kind: "visualization",
			title: "Visualization",
			visualizationRows: [],
		});
		const rows = [
			{
				id: "row-1",
				widgets: [
					{
						...createVisualizationWidget("widget-1"),
						id: "widget-1",
						title: "Dimmer",
						type: "bar" as const,
						source: { kind: "raw_dmx" as const, universe: 1, address: 12 },
						operation: "multiply" as const,
						factor: 1,
						displayScale: "percent" as const,
						minimum: 0,
						maximum: 100,
						bar: { orientation: "vertical" as const },
					},
				],
			},
		];
		const edited = appReducer(added, {
			type: "SET_PANE_VISUALIZATION_ROWS",
			id: pane.id,
			rows,
		});
		expect(edited.desks[0].panes[0].visualizationRows).toEqual(rows);

		const hydrated = appReducer(edited, {
			type: "HYDRATE_LAYOUT",
			desks: edited.desks,
			activeDeskId: "visualization-test",
		});
		expect(hydrated.desks[0].panes[0].visualizationRows).toMatchObject(rows);
	});
});

describe("appReducer dock navigation and File Manager returns", () => {
	it("restores the last desk and built-in when switching dock sections", () => {
		const patch = appReducer(initialState, { type: "OPEN_DESK", id: "patch" });
		const groups = appReducer(patch, { type: "OPEN_BUILTIN", kind: "groups" });
		const desks = appReducer(groups, { type: "SET_DOCK_MODE", mode: "desks" });
		expect(desks.activeDeskId).toBe("patch");
		expect(desks.builtIn).toBeNull();
		const builtIns = appReducer(desks, {
			type: "SET_DOCK_MODE",
			mode: "builtins",
		});
		expect(builtIns.builtIn).toBe("groups");
	});

	it("closes File Manager back to the built-in that launched it", () => {
		const setup = appReducer(initialState, {
			type: "OPEN_BUILTIN",
			kind: "setup",
		});
		const manager = appReducer(setup, {
			type: "OPEN_BUILTIN",
			kind: "file_manager",
		});
		expect(manager.fileManagerReturn).toMatchObject({
			dockMode: "builtins",
			builtIn: "setup",
		});
		expect(manager.lastBuiltIn).toBe("setup");

		const closed = appReducer(manager, { type: "CLOSE_FILE_MANAGER" });
		expect(closed).toMatchObject({
			dockMode: "builtins",
			builtIn: "setup",
			fileManagerReturn: null,
		});
	});

	it("closes File Manager back to the active Desktop that launched it", () => {
		const playback = appReducer(initialState, {
			type: "OPEN_DESK",
			id: "playback",
		});
		const manager = appReducer(playback, {
			type: "OPEN_BUILTIN",
			kind: "file_manager",
		});
		const closed = appReducer(manager, { type: "CLOSE_FILE_MANAGER" });
		expect(closed).toMatchObject({
			dockMode: "desks",
			activeDeskId: "playback",
			builtIn: null,
			fileManagerReturn: null,
		});
	});
});

describe("appReducer playback, preset, and record workflows", () => {
	it("configures playback rows and columns within desk limits", () => {
		const configured = appReducer(initialState, {
			type: "SET_PLAYBACK_LAYOUT",
			columns: 20,
			rows: 3,
		});
		expect(configured.playbackColumns).toBe(20);
		expect(configured.playbackRows).toBe(3);
		const clamped = appReducer(configured, {
			type: "SET_PLAYBACK_LAYOUT",
			columns: 99,
			rows: 8,
		});
		expect(clamped.playbackColumns).toBe(32);
		expect(clamped.playbackRows).toBe(3);
	});

	it("moves between playback executor pages and clamps at the ends", () => {
		expect(
			appReducer(initialState, { type: "SET_PLAYBACK_PAGE", page: 3 })
				.playbackPage,
		).toBe(3);
		expect(
			appReducer(initialState, { type: "SET_PLAYBACK_PAGE", page: 999 })
				.playbackPage,
		).toBe(126);
		expect(
			appReducer(initialState, { type: "SET_PLAYBACK_PAGE", page: -1 })
				.playbackPage,
		).toBe(0);
	});

	it("keeps the preset family while navigating between built-ins", () => {
		const intensity = appReducer(initialState, {
			type: "SET_PRESET_FAMILY",
			family: "Intensity",
		});
		const groups = appReducer(intensity, {
			type: "OPEN_BUILTIN",
			kind: "groups",
		});
		const presets = appReducer(groups, {
			type: "OPEN_BUILTIN",
			kind: "presets",
		});
		expect(presets.presetFamily).toBe("Intensity");
	});

	it("arms and cancels the shared store workflow explicitly", () => {
		const armed = appReducer(initialState, {
			type: "SET_STORE_ARMED",
			value: true,
		});
		expect(armed.storeArmed).toBe(true);
		expect(
			appReducer(armed, { type: "SET_STORE_ARMED", value: false }).storeArmed,
		).toBe(false);
	});

	it("keeps Update and Record mutually exclusive", () => {
		const updating = appReducer(initialState, {
			type: "SET_UPDATE_ARMED",
			value: true,
		});
		expect(updating.updateArmed).toBe(true);
		expect(updating.storeArmed).toBe(false);
		const recording = appReducer(updating, {
			type: "SET_STORE_ARMED",
			value: true,
		});
		expect(recording.storeArmed).toBe(true);
		expect(recording.updateArmed).toBe(false);
	});
});

describe("appReducer Stage and Development pane settings", () => {
	it("stores the displayed Channel mode independently on its pane", () => {
		const pane = {
			id: "channels-a",
			kind: "channels" as const,
			title: "Channels",
			x: 0,
			y: 0,
			width: 8,
			height: 8,
		};
		const state = {
			...initialState,
			desks: initialState.desks.map((desk) =>
				desk.id === initialState.activeDeskId
					? { ...desk, panes: [...desk.panes, pane] }
					: desk,
			),
		};
		const updated = appReducer(state, {
			type: "SET_PANE_CHANNEL_DISPLAY_MODE",
			id: pane.id,
			mode: "all",
		});
		expect(
			updated.desks
				.find((desk) => desk.id === updated.activeDeskId)
				?.panes.find((candidate) => candidate.id === pane.id)
				?.channelDisplayMode,
		).toBe("all");

		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: [
				{
					id: "channels-desk",
					name: "Channels",
					panes: [{ ...pane, channelDisplayMode: "future-mode" as never }],
				},
			],
			activeDeskId: "channels-desk",
		});
		expect(hydrated.desks[0].panes[0].channelDisplayMode).toBe("intensity");
	});

	it("updates stage presentation options and clamps environment brightness", () => {
		const hidden = appReducer(initialState, {
			type: "SET_STAGE_OPTIONS",
			groupsVisible: false,
			showSelection: false,
			showFloorGrid: false,
			side2d: "front",
			environmentBrightness: 3,
		});
		expect(hidden.stageGroupsVisible).toBe(false);
		expect(hidden.stageShowSelection).toBe(false);
		expect(hidden.stageShowFloorGrid).toBe(false);
		expect(hidden.stage2dSide).toBe("front");
		expect(hidden.stageEnvironmentBrightness).toBe(2);
		expect(
			appReducer(hidden, {
				type: "SET_STAGE_OPTIONS",
				environmentBrightness: -1,
			}).stageEnvironmentBrightness,
		).toBe(0);
	});

	it("defaults new visualizers to 5% ambient light without replacing persisted values", () => {
		expect(initialState.stageEnvironmentBrightness).toBe(0.05);

		for (const stageEnvironmentBrightness of [0, 0.35, 1]) {
			const hydrated = appReducer(initialState, {
				type: "HYDRATE_LAYOUT",
				desks: initialState.desks,
				activeDeskId: initialState.activeDeskId,
				windowSettings: { stageEnvironmentBrightness },
			});
			expect(hydrated.stageEnvironmentBrightness).toBe(
				stageEnvironmentBrightness,
			);
		}

		const legacy = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
		});
		expect(legacy.stageEnvironmentBrightness).toBe(0.05);
	});

	it("stores the 2D side independently on a Stage pane", () => {
		const updated = appReducer(initialState, {
			type: "SET_PANE_STAGE_OPTION",
			id: "stage",
			option: "stage2dSide",
			value: "left",
		});
		expect(
			updated.desks
				.find((desk) => desk.id === updated.activeDeskId)
				?.panes.find((pane) => pane.id === "stage")?.stage2dSide,
		).toBe("left");
		expect(updated.stage2dSide).toBe("top");
	});

	it("stores the Stage view independently on each Stage pane", () => {
		const updated = appReducer(initialState, {
			type: "SET_PANE_STAGE_OPTION",
			id: "stage",
			option: "stageView",
			value: "3d",
		});
		expect(
			updated.desks
				.find((desk) => desk.id === updated.activeDeskId)
				?.panes.find((pane) => pane.id === "stage")?.stageView,
		).toBe("3d");
		expect(updated.stageView).toBe("2d");
	});

	it("stores bounded Ultra fog character independently on each Visualizer pane", () => {
		let updated = initialState;
		for (const [option, value] of [
			["lampFogCloudiness", 0.25],
			["lampFogTurbulence", 0.5],
			["laserFogCloudiness", 0.75],
			["laserFogTurbulence", 2],
		] as const)
			updated = appReducer(updated, {
				type: "SET_PANE_FOG_VARIATION",
				id: "stage",
				option,
				value,
			});
		const stage = updated.desks
			.find((desk) => desk.id === updated.activeDeskId)
			?.panes.find((pane) => pane.id === "stage");
		expect(stage).toMatchObject({
			lampFogCloudiness: 0.25,
			lampFogTurbulence: 0.5,
			laserFogCloudiness: 0.75,
			laserFogTurbulence: 1,
		});
	});

	it("stores the complete Fixture Sheet configuration independently on each pane", () => {
		const options = appReducer(initialState, {
			type: "SET_PANE_FIXTURE_OPTIONS",
			id: "fixtures",
			options: {
				includedHeads: "no-sub-heads",
				order: "active",
				cueListId: "front",
				columns: ["id", "name", "patch"],
				showType: false,
			},
		});
		const pane = options.desks
			.find((desk) => desk.id === options.activeDeskId)
			?.panes.find((candidate) => candidate.id === "fixtures");
		expect(pane).toMatchObject({
			fixtureSheetIncludedHeads: "no-sub-heads",
			fixtureSheetOrder: "active",
			fixtureSheetCueListId: "front",
			fixtureSheetColumns: ["id", "name", "patch"],
			fixtureSheetShowType: false,
		});
		expect(options.fixtureSheetOrder).toBe(initialState.fixtureSheetOrder);
	});

	it("retires persisted Layout panes and built-ins with one actionable notice", () => {
		const desks = [
			{
				id: "test",
				name: "Test",
				panes: [
					{
						id: "layout-a",
						kind: "layout",
						title: "Front Layout",
						x: 0,
						y: 0,
						width: 8,
						height: 8,
						layoutGroupId: "1",
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: desks as typeof initialState.desks,
			activeDeskId: "test",
			windowSettings: {
				builtIn: "layout",
				lastBuiltIn: "layout",
				layoutGroupId: "2",
			},
		});
		expect(hydrated.desks[0].panes).toEqual([]);
		expect(hydrated.builtIn).toBeNull();
		expect(hydrated.lastBuiltIn).toBe(initialState.lastBuiltIn);
		expect(hydrated.layoutMigrationNotice).toBe(true);
		expect("layoutGroupId" in hydrated).toBe(false);

		const dismissed = appReducer(hydrated, {
			type: "DISMISS_LAYOUT_MIGRATION_NOTICE",
		});
		expect(dismissed.layoutMigrationNotice).toBe(false);
	});

	it("drops retired Development panes from persisted layouts", () => {
		const desks = [
			{
				id: "test",
				name: "Test",
				panes: [
					{
						id: "development",
						kind: "development",
						title: "Development",
						x: 1,
						y: 1,
						width: 8,
						height: 8,
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: desks as unknown as typeof initialState.desks,
			activeDeskId: "test",
			windowSettings: {
				builtIn: "development",
				lastBuiltIn: "development",
			} as never,
		});
		expect(hydrated.desks[0].panes).toEqual([]);
		expect(hydrated.builtIn).toBeNull();
		expect(hydrated.lastBuiltIn).toBe(initialState.lastBuiltIn);
	});
});

describe("appReducer Cues pane settings", () => {
	it("persists compact rows per Cues pane while legacy panes remain standard", () => {
		const state = {
			...initialState,
			activeDeskId: "cues",
			desks: [
				{
					id: "cues",
					name: "Cues",
					panes: [
						{
							id: "cues-1",
							kind: "cues" as const,
							title: "One",
							x: 1,
							y: 1,
							width: 12,
							height: 12,
						},
						{
							id: "cues-2",
							kind: "cues" as const,
							title: "Two",
							x: 13,
							y: 1,
							width: 12,
							height: 12,
						},
					],
				},
			],
		};
		expect(state.desks[0].panes[0]).not.toHaveProperty("cueListCompactRows");

		const compact = appReducer(state, {
			type: "SET_PANE_CUELIST_COMPACT_ROWS",
			id: "cues-1",
			value: true,
		});
		expect(compact.desks[0].panes[0].cueListCompactRows).toBe(true);
		expect(compact.desks[0].panes[1].cueListCompactRows).toBeUndefined();
	});

	it("persists the current or next Cue information block per pane", () => {
		const state = {
			...initialState,
			activeDeskId: "cues",
			desks: [
				{
					id: "cues",
					name: "Cues",
					panes: [
						{
							id: "cues-1",
							kind: "cues" as const,
							title: "One",
							x: 1,
							y: 1,
							width: 12,
							height: 12,
						},
					],
				},
			],
		};
		const next = appReducer(state, {
			type: "SET_PANE_CUE_INFORMATION_BLOCK",
			id: "cues-1",
			value: "next",
		});
		expect(next.desks[0].panes[0].cueInformationBlock).toBe("next");
	});

	it("persists Cue sidebar visibility while older pane layouts keep it visible", () => {
		const desks = [
			{
				id: "cues",
				name: "Cues",
				panes: [
					{
						id: "cues-1",
						kind: "cues" as const,
						title: "Cues · Main",
						x: 1,
						y: 1,
						width: 12,
						height: 12,
					},
				],
			},
		];
		const legacy = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "cues",
		});
		expect(legacy.desks[0].panes[0].showCueSidebar ?? true).toBe(true);

		const hidden = appReducer(legacy, {
			type: "SET_PANE_CUE_SIDEBAR",
			id: "cues-1",
			value: false,
		});
		expect(hidden.desks[0].panes[0].showCueSidebar).toBe(false);
	});

	it("persists each Cues pane's fixed or follow-selection display choice", () => {
		const desks = [
			{
				id: "cues",
				name: "Cues",
				panes: [
					{
						id: "cues-1",
						kind: "cues" as const,
						title: "Cues 1",
						x: 1,
						y: 1,
						width: 12,
						height: 9,
					},
					{
						id: "cues-2",
						kind: "cues" as const,
						title: "Cues 2",
						x: 13,
						y: 1,
						width: 12,
						height: 9,
					},
				],
			},
		];
		const legacy = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "cues",
		});
		expect(legacy.desks[0].panes[0].cueListSource ?? "fixed").toBe("fixed");

		const fixed = appReducer(legacy, {
			type: "SET_PANE_CUELIST",
			id: "cues-1",
			number: 7,
		});
		const followed = appReducer(fixed, {
			type: "SET_PANE_CUELIST",
			id: "cues-2",
			source: "follow-selection",
		});
		expect(followed.desks[0].panes[0]).toEqual(
			expect.objectContaining({
				cueListSource: "fixed",
				fixedCueListNumber: 7,
			}),
		);
		expect(followed.desks[0].panes[1]).toEqual(
			expect.objectContaining({ cueListSource: "follow-selection" }),
		);
		expect(followed.desks[0].panes[1]).not.toHaveProperty("fixedCueListNumber");
	});
});

describe("appReducer Fixture Sheet pane settings", () => {
	it("persists pane-local filter and Compact mode independently", () => {
		const state = {
			...initialState,
			activeDeskId: "fixtures",
			desks: [
				{
					id: "fixtures",
					name: "Fixtures",
					panes: [
						{
							id: "fixtures-1",
							kind: "fixtures" as const,
							title: "Fixture Sheet",
							x: 1,
							y: 1,
							width: 12,
							height: 18,
						},
						{
							id: "fixtures-2",
							kind: "fixtures" as const,
							title: "Fixture Sheet 2",
							x: 13,
							y: 1,
							width: 12,
							height: 18,
						},
					],
				},
			],
		};
		const filtered = appReducer(state, {
			type: "SET_PANE_FIXTURE_ACTIVE_ONLY",
			id: "fixtures-1",
			value: true,
		});
		expect(filtered.desks[0].panes[0].fixtureSheetActiveOnly).toBe(true);
		const compact = appReducer(filtered, {
			type: "SET_PANE_FIXTURE_COMPACT_MODE",
			id: "fixtures-1",
			mode: "icon-only",
		});
		expect(compact.desks[0].panes[0].fixtureSheetCompactMode).toBe("icon-only");
		expect(compact.desks[0].panes[1].fixtureSheetCompactMode).toBeUndefined();

		const restored = appReducer(compact, {
			type: "HYDRATE_FIXTURE_SHEET_COMPACT_MODES",
			builtIn: "text-only",
			desktops: {
				fixtures: {
					"fixtures-1": "icon-only",
					"fixtures-2": "text-only",
				},
			},
		});
		expect(restored.fixtureSheetCompactMode).toBe("text-only");
		expect(restored.desks[0].panes[0].fixtureSheetCompactMode).toBe(
			"icon-only",
		);
		expect(restored.desks[0].panes[1].fixtureSheetCompactMode).toBe(
			"text-only",
		);

		const otherDeskDefault = appReducer(restored, {
			type: "HYDRATE_FIXTURE_SHEET_COMPACT_MODES",
			builtIn: "off",
			desktops: {},
		});
		expect(otherDeskDefault.fixtureSheetCompactMode).toBe("off");
		expect(otherDeskDefault.desks[0].panes[0].fixtureSheetCompactMode).toBe(
			"off",
		);
		expect(otherDeskDefault.desks[0].panes[1].fixtureSheetCompactMode).toBe(
			"off",
		);
	});
});

describe("appReducer Text Editor pane settings", () => {
	it("persists only non-authoritative Text Editor view state in the pane layout", () => {
		const desks = [
			{
				id: "notes",
				name: "Notes",
				panes: [
					{
						id: "editor",
						kind: "text_editor" as const,
						title: "Text Editor",
						x: 1,
						y: 1,
						width: 8,
						height: 8,
						textFileRoot: "shows",
						textFilePath: "run.md",
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "notes",
		});
		const updated = appReducer(hydrated, {
			type: "SET_TEXT_EDITOR_VIEW",
			id: "editor",
			root: "shows",
			path: "run.md",
			selectionStart: 12,
			selectionEnd: 16,
			scrollTop: 240,
		});
		expect(updated.desks[0].panes[0].textEditorView).toEqual({
			root: "shows",
			path: "run.md",
			selectionStart: 12,
			selectionEnd: 16,
			scrollTop: 240,
		});
		expect(updated.desks[0].panes[0]).not.toHaveProperty("text");
	});

	it("persists Text Editor pane mode and read-only settings while older layouts retain safe defaults", () => {
		const desks = [
			{
				id: "notes",
				name: "Notes",
				panes: [
					{
						id: "editor",
						kind: "text_editor" as const,
						title: "Text Editor",
						x: 1,
						y: 1,
						width: 8,
						height: 8,
					},
				],
			},
		];
		const legacy = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "notes",
		});
		expect(legacy.desks[0].panes[0].textEditorReadOnly ?? false).toBe(false);
		expect(legacy.desks[0].panes[0].textEditorMode ?? "plain").toBe("plain");

		const readOnly = appReducer(legacy, {
			type: "SET_TEXT_EDITOR_SETTINGS",
			id: "editor",
			readOnly: true,
		});
		const rendered = appReducer(readOnly, {
			type: "SET_TEXT_EDITOR_SETTINGS",
			id: "editor",
			mode: "split",
		});
		expect(rendered.desks[0].panes[0]).toEqual(
			expect.objectContaining({
				textEditorReadOnly: true,
				textEditorMode: "split",
			}),
		);
	});
});

describe("appReducer legacy pane layout hydration", () => {
	it("hydrates legacy pane layouts without requiring newly added pane fields", () => {
		const desks = [
			{
				id: "legacy-workspace",
				name: "Legacy workspace",
				panes: [
					{
						id: "virtual",
						kind: "virtual_playbacks" as const,
						title: "Virtual Playbacks",
						x: 1,
						y: 1,
						width: 8,
						height: 8,
					},
					{
						id: "files",
						kind: "file_manager" as const,
						title: "File Manager",
						x: 9,
						y: 1,
						width: 8,
						height: 8,
					},
					{
						id: "notes",
						kind: "text_editor" as const,
						title: "Text Editor",
						x: 17,
						y: 1,
						width: 8,
						height: 8,
					},
				],
			},
		];

		const legacy = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "legacy-workspace",
		});
		const [virtual, files, notes] = legacy.desks[0].panes;

		expect(virtual.virtualPlaybackRows ?? 2).toBe(2);
		expect(virtual.virtualPlaybackColumns ?? 2).toBe(2);
		expect(virtual.virtualPlaybackPageMode ?? "follow_main").toBe(
			"follow_main",
		);
		expect(virtual.virtualPlaybackPinnedPage ?? 1).toBe(1);
		expect(virtual.virtualPlaybackCells ?? []).toEqual([]);
		expect(virtual.virtualPlaybackExclusionZones ?? []).toEqual([]);
		expect(files.fileManagerShowHidden ?? false).toBe(false);
		expect(notes.textEditorReadOnly ?? false).toBe(false);
		expect(notes.textEditorMode ?? "plain").toBe("plain");
	});

	it("accepts a full 20 by 15 Virtual Playback page and bounds larger products", () => {
		const desks = [
			{
				id: "virtual-workspace",
				name: "Virtual workspace",
				panes: [
					{
						id: "virtual",
						kind: "virtual_playbacks" as const,
						title: "Virtual Playbacks",
						x: 1,
						y: 1,
						width: 8,
						height: 8,
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "virtual-workspace",
		});
		const ordinary = appReducer(hydrated, {
			type: "SET_VIRTUAL_PLAYBACK_GRID",
			id: "virtual",
			rows: 20,
			columns: 15,
			changed: "columns",
		});
		expect(ordinary.desks[0].panes[0]).toMatchObject({
			virtualPlaybackRows: 20,
			virtualPlaybackColumns: 15,
		});

		const bounded = appReducer(ordinary, {
			type: "SET_VIRTUAL_PLAYBACK_GRID",
			id: "virtual",
			rows: 20,
			columns: 500,
			changed: "columns",
		});
		expect(bounded.desks[0].panes[0]).toMatchObject({
			virtualPlaybackRows: 20,
			virtualPlaybackColumns: 15,
		});
	});

	it("persists Follow Main and bounded Pinned page settings", () => {
		const desks = [
			{
				id: "virtual-workspace",
				name: "Virtual workspace",
				panes: [
					{
						id: "virtual-surface",
						kind: "virtual_playbacks" as const,
						title: "Virtual Playbacks",
						x: 1,
						y: 1,
						width: 8,
						height: 8,
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "virtual-workspace",
		});
		const pinned = appReducer(hydrated, {
			type: "SET_VIRTUAL_PLAYBACK_PAGE_MODE",
			id: "virtual-surface",
			mode: "pinned",
			pinnedPage: 999,
		});
		expect(pinned.desks[0].panes[0]).toMatchObject({
			id: "virtual-surface",
			virtualPlaybackPageMode: "pinned",
			virtualPlaybackPinnedPage: 127,
		});
		const followed = appReducer(pinned, {
			type: "SET_VIRTUAL_PLAYBACK_PAGE_MODE",
			id: "virtual-surface",
			mode: "follow_main",
		});
		expect(followed.desks[0].panes[0]).toMatchObject({
			id: "virtual-surface",
			virtualPlaybackPageMode: "follow_main",
			virtualPlaybackPinnedPage: 127,
		});
	});
});

describe("appReducer built-in window settings hydration", () => {
	it("hydrates persisted built-in window settings without requiring them in older layouts", () => {
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
			windowSettings: {
				builtIn: "dmx",
				dockMode: "builtins",
				stageView: "3d",
				dmxDotSize: "large",
				fixtureSheetColumns: ["id", "name", "dimmer"],
				fixtureSheetShowType: false,
				fixtureSheetShowPatch: false,
				fixtureSheetShowSubheads: false,
				fixtureSheetShowMasterHeads: true,
				fixtureGroupsVisible: false,
				presetGroupsVisible: false,
			},
		});
		expect(hydrated.builtIn).toBe("dmx");
		expect(hydrated.dockMode).toBe("builtins");
		expect(hydrated.stageView).toBe("3d");
		expect(hydrated.dmxDotSize).toBe("large");
		expect(hydrated.fixtureSheetColumns).toEqual(["id", "name", "intensity"]);
		expect(hydrated.fixtureSheetCompactMode).toBe("off");
		expect(hydrated.fixtureSheetShowType).toBe(false);
		expect(hydrated.fixtureSheetIncludedHeads).toBe("no-sub-heads");
		expect(hydrated.fixtureGroupsVisible).toBe(false);
		expect(hydrated.presetGroupsVisible).toBe(false);
		const legacy = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
		});
		expect(legacy.stageView).toBe(initialState.stageView);
		expect(legacy.stageShowFloorGrid).toBe(true);
		expect(legacy.stage2dSide).toBe("top");
		expect(legacy.fixtureSheetColumns).toEqual(
			initialState.fixtureSheetColumns,
		);
		expect(legacy.fixtureSheetShowType).toBe(true);
		expect(legacy.fixtureSheetIncludedHeads).toBe("all");

		const current = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
			windowSettings: {
				fixtureSheetIncludedHeads: "no-master-heads",
				fixtureSheetCompactMode: "text-only",
			},
		});
		expect(current.fixtureSheetIncludedHeads).toBe("no-master-heads");
		expect(current.fixtureSheetCompactMode).toBe("off");

		// A layout from before the renderer drew every Stage carries a render style rather than a
		// side. There is nothing in one to convert, so it is dropped and the Stage opens on the
		// plan, which is what a 2D Stage was.
		const oldStageStyle = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
			windowSettings: {
				stageRenderQuality: "improved_beams",
			},
		});
		expect(oldStageStyle.stage2dSide).toBe("top");

		const oldPatchDetail = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
			windowSettings: {
				fixtureSheetColumns: ["id", "name", "dimmer"],
				fixtureSheetShowPatch: true,
			},
		});
		expect(oldPatchDetail.fixtureSheetColumns).toEqual([
			"id",
			"name",
			"patch",
			"intensity",
		]);
	});
});

describe("appReducer Fixture Sheet and preset pane migrations", () => {
	it("keeps at least one valid fixture-sheet column when updating or migrating settings", () => {
		const oneColumn = appReducer(initialState, {
			type: "SET_FIXTURE_SHEET_OPTIONS",
			columns: ["name"],
		});
		expect(oneColumn.fixtureSheetColumns).toEqual(["name"]);
		const rejectedEmpty = appReducer(oneColumn, {
			type: "SET_FIXTURE_SHEET_OPTIONS",
			columns: [],
		});
		expect(rejectedEmpty.fixtureSheetColumns).toEqual(["name"]);

		const migrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: initialState.desks,
			activeDeskId: initialState.activeDeskId,
			windowSettings: { fixtureSheetColumns: [] },
		});
		expect(migrated.fixtureSheetColumns).toEqual(
			initialState.fixtureSheetColumns,
		);
	});

	it("persists preset family independently on a preset pane and migrates legacy panes", () => {
		const desks = [
			{
				id: "test",
				name: "Test",
				panes: [
					{
						id: "pool",
						kind: "presets" as const,
						title: "Presets",
						x: 1,
						y: 1,
						width: 6,
						height: 6,
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "test",
		});
		expect(hydrated.desks[0].panes[0].presetFamily).toBe("Mixed");
		const color = appReducer(hydrated, {
			type: "SET_PANE_PRESET_FAMILY",
			id: "pool",
			family: "Color",
		});
		expect(color.desks[0].panes[0].presetFamily).toBe("Color");
		expect(color.presetFamily).toBe("Mixed");
	});

	it("persists a bounded column count on a Group Pool pane", () => {
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: [
				{
					id: "groups",
					name: "Groups",
					panes: [
						{
							id: "group-pool",
							kind: "groups",
							title: "Group Pool",
							x: 1,
							y: 1,
							width: 8,
							height: 18,
						},
					],
				},
			],
			activeDeskId: "groups",
		});
		const sevenColumns = appReducer(hydrated, {
			type: "SET_PANE_POOL_COLUMNS",
			id: "group-pool",
			value: 7,
		});
		expect(sevenColumns.desks[0].panes[0].poolColumns).toBe(7);
		const bounded = appReducer(sevenColumns, {
			type: "SET_PANE_POOL_COLUMNS",
			id: "group-pool",
			value: 99,
		});
		expect(bounded.desks[0].panes[0].poolColumns).toBe(24);
	});

	it("migrates legacy Programming preset panes and All family state to Mixed", () => {
		const desks = [
			{
				id: "programming",
				name: "Programming",
				panes: [
					{
						id: "presets",
						kind: "presets" as const,
						title: "All Presets",
						x: 1,
						y: 1,
						width: 9,
						height: 18,
						presetFamily: "All" as never,
					},
				],
			},
		];
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks,
			activeDeskId: "programming",
			windowSettings: { presetFamily: "All" as never },
		});
		expect(hydrated.desks[0].panes[0]).toMatchObject({
			title: "Mixed Presets",
			presetFamily: "Mixed",
		});
		expect(hydrated.presetFamily).toBe("Mixed");
	});
});

describe("appReducer pool and Set configuration", () => {
	it("keeps pool colors and Set configuration mode independently configurable", () => {
		const plain = appReducer(initialState, {
			type: "SET_PRESET_POOL_COLORS",
			value: false,
		});
		expect(plain.presetPoolColors).toBe(false);
		const armed = appReducer(plain, {
			type: "SET_PRESET_SET_ARMED",
			value: true,
		});
		expect(armed.presetSetArmed).toBe(true);
	});

	it("keeps the selected pool playback while Set waits for a fader target", () => {
		const armed = appReducer(initialState, {
			type: "SET_CUELIST_SET_ARMED",
			value: true,
		});
		const selected = appReducer(armed, {
			type: "SET_CUELIST_SET_TARGET",
			value: 42,
		});
		expect(selected.cueListSetArmed).toBe(true);
		expect(selected.cueListSetTarget).toBe(42);
		expect(
			appReducer(selected, { type: "SET_CUELIST_SET_ARMED", value: false })
				.cueListSetTarget,
		).toBeNull();
	});

	it("arms and clears playback configuration Set selection", () => {
		const armed = appReducer(initialState, {
			type: "SET_PLAYBACK_SET_ARMED",
			value: true,
		});
		expect(armed.playbackSetArmed).toBe(true);
		expect(
			appReducer(armed, { type: "SET_PLAYBACK_SET_ARMED", value: false })
				.playbackSetArmed,
		).toBe(false);
	});
});

describe("appReducer Cuelist built-in navigation", () => {
	it("returns the Cuelists built-in to the pool when its button is clicked from a Cuelist", () => {
		const opened = appReducer(initialState, {
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		const inside = appReducer(opened, {
			type: "OPEN_BUILTIN_CUELIST",
			number: 7,
		});
		const returned = appReducer(inside, {
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		expect(returned).toMatchObject({
			builtIn: "cuelists",
			cuelistBuiltInView: "pool",
			cuelistBuiltInNumber: 7,
		});
	});

	it("reopens the remembered Cuelist from another screen before returning to the pool", () => {
		const opened = appReducer(initialState, {
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		const inside = appReducer(opened, {
			type: "OPEN_BUILTIN_CUELIST",
			number: 12,
		});
		const elsewhere = appReducer(inside, {
			type: "OPEN_BUILTIN",
			kind: "fixtures",
		});
		const reopened = appReducer(elsewhere, {
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		expect(reopened).toMatchObject({
			builtIn: "cuelists",
			cuelistBuiltInView: "cues",
			cuelistBuiltInNumber: 12,
		});
		expect(
			appReducer(reopened, { type: "OPEN_BUILTIN", kind: "cuelists" })
				.cuelistBuiltInView,
		).toBe("pool");
	});
});
