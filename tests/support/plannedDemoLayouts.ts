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
					pane(
						"busking-playbacks",
						"virtual_playbacks",
						"Virtual Playbacks",
						13,
						1,
						12,
						18,
						{
							virtualPlaybackRows: 5,
							virtualPlaybackColumns: 10,
							virtualPlaybackPageMode: "pinned",
							virtualPlaybackPinnedPage: 1,
						},
					),
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
	const existing = (await api.showObjects<any>(showId, "user_layout")).find(
		(layout) => layout.id === sessionId,
	);
	const body = plannedDemoLayout();
	const visibleBusking = existing?.body.desks?.find(
		(desk: any) => desk.name === "Busking",
	);
	if (visibleBusking) {
		const busking = body.desks.find((desk) => desk.id === "busking");
		if (!busking) throw new Error("Canonical Busking desktop is unavailable");
		const visibleVirtualPlaybacks = visibleBusking.panes?.find(
			(pane: any) => pane.kind === "virtual_playbacks",
		);
		busking.id = visibleBusking.id;
		if (visibleVirtualPlaybacks) {
			const plannedVirtualPlaybacks = busking.panes.find(
				(pane) => pane.kind === "virtual_playbacks",
			);
			if (plannedVirtualPlaybacks)
				plannedVirtualPlaybacks.id = visibleVirtualPlaybacks.id;
		}
		body.activeDeskId = visibleBusking.id;
	}
	await api.seedShowObject(
		showId,
		"user_layout",
		sessionId,
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
	const existing = (await api.showObjects<any>(showId, "user_layout")).find(
		(layout) => layout.id === sessionId,
	);
	const body = plannedDemoLayout();
	body.activeDeskId = "group-programming";
	await api.seedShowObject(
		showId,
		"user_layout",
		sessionId,
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
