import type { ApiDriver } from "../bench/core/api";

export function plannedDemoLayout() {
	return {
		activeDeskId: "busking",
		desks: [
			{
				id: "group-programming",
				name: "Group Programming",
				panes: [
					pane(
						"group-programming-fixtures",
						"fixtures",
						"Fixture Sheet",
						1,
						1,
						16,
						18,
					),
					pane(
						"group-programming-groups",
						"groups",
						"Group Pool",
						17,
						1,
						8,
						18,
						{ poolColumns: 7 },
					),
				],
			},
			{
				id: "busking",
				name: "Busking",
				panes: [
					pane("busking-groups", "groups", "Groups", 1, 1, 6, 9),
					pane("busking-color", "presets", "Color Presets", 7, 1, 6, 9, {
						presetFamily: "Color",
					}),
					pane("busking-position", "presets", "Position Presets", 1, 10, 6, 9, {
						presetFamily: "Position",
					}),
					pane("busking-beam", "presets", "Beam Presets", 7, 10, 6, 9, {
						presetFamily: "Beam",
					}),
					pane("busking-playbacks", "playbacks", "Playbacks", 13, 1, 12, 18, {
						surface_id: "pane:busking-playbacks",
					}),
				],
			},
			{
				id: "cue-programming",
				name: "Cue Programming",
				panes: [
					pane(
						"cue-programming-fixtures",
						"fixtures",
						"Fixture Sheet",
						1,
						1,
						14,
						18,
						{ fixtureSheetActiveOnly: true },
					),
					pane(
						"cue-programming-pool",
						"cuelist_pool",
						"Cuelist Pool",
						15,
						1,
						10,
						9,
					),
					pane(
						"cue-programming-detail",
						"cues",
						"Cues · Cuelist",
						15,
						10,
						10,
						9,
						{ cueListSource: "follow-selection", showCueSidebar: true },
					),
				],
			},
			{
				id: "programming",
				name: "Programming",
				panes: [
					pane(
						"programming-fixtures",
						"fixtures",
						"Fixture Sheet",
						1,
						1,
						12,
						18,
					),
					pane("programming-stage", "stage", "Stage", 13, 1, 12, 12, {
						stageView: "3d",
						followPreload: false,
						showBeamGuides: true,
					}),
					pane("programming-dmx", "dmx", "DMX Output", 13, 13, 12, 6),
				],
			},
			{
				id: "theater",
				name: "Theater",
				panes: [
					pane("theater-cuelist", "cue_list", "Cuelist", 1, 1, 12, 18, {
						showCueSidebar: true,
						cueListSource: "follow-selection",
					}),
					pane("theater-text", "text_editor", "Theater Script", 13, 1, 12, 18, {
						textEditorMode: "split",
						textEditorReadOnly: false,
					}),
				],
			},
		],
	};
}

export async function installPlannedDemoLayout(api: ApiDriver, showId: string) {
	const sessionId = api.session?.session_id;
	if (!sessionId)
		throw new Error(
			"Plan 76 desktop generation requires an authenticated user",
		);
	const layouts = await api.showObjects<any>(showId, "user_layout");
	// The canonical asset carries one portable layout; regeneration adopts it across bench sessions.
	const existing =
		layouts.find((layout) => layout.id === sessionId) ?? layouts[0];
	await api.request(
		"POST",
		"/api/v2/playback-surfaces/pane%3Abusking-playbacks/update",
		{
			request_id: crypto.randomUUID(),
			name: "Busking Playbacks",
			page_source: { type: "pinned", page: 1 },
			layout: {
				rows: Array.from({ length: 5 }, (_, row) => ({
					first_position: 401 + row * 10,
					count: 10,
					presentation: "buttons",
					button_count: 3,
				})),
			},
		},
		true,
	);
	const body = plannedDemoLayout();
	const visibleBusking = existing?.body.desks?.find(
		(desk: any) => desk.name === "Busking",
	);
	if (visibleBusking) {
		const busking = body.desks.find((desk) => desk.id === "busking");
		if (!busking) throw new Error("Canonical Busking desktop is unavailable");
		const visiblePlaybacks = visibleBusking.panes?.find(
			(pane: any) => pane.kind === "playbacks",
		);
		busking.id = visibleBusking.id;
		if (visiblePlaybacks) {
			const plannedPlaybacks = busking.panes.find(
				(pane) => pane.kind === "playbacks",
			);
			if (plannedPlaybacks) plannedPlaybacks.id = visiblePlaybacks.id;
		}
		body.activeDeskId = visibleBusking.id;
	}
	await api.seedShowObject(
		showId,
		"user_layout",
		existing?.id ?? sessionId,
		body,
		existing?.revision ?? 0,
	);
	return body;
}

export async function installPlannedDemoGroupProgrammingLayout(
	api: ApiDriver,
	showId: string,
) {
	const sessionId = api.session?.session_id;
	if (!sessionId)
		throw new Error("Group Programming desktop requires an authenticated user");
	const layouts = await api.showObjects<any>(showId, "user_layout");
	// The canonical asset carries one portable layout; regeneration adopts it across bench sessions.
	const existing =
		layouts.find((layout) => layout.id === sessionId) ?? layouts[0];
	const body = plannedDemoLayout();
	body.activeDeskId = "group-programming";
	await api.seedShowObject(
		showId,
		"user_layout",
		existing?.id ?? sessionId,
		body,
		existing?.revision ?? 0,
	);
	return body;
}

function pane(
	id: string,
	kind: string,
	title: string,
	x: number,
	y: number,
	width: number,
	height: number,
	options: Record<string, unknown> = {},
) {
	return { id, kind, title, x, y, width, height, ...options };
}
