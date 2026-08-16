import type { ApiDriver } from "../bench/core/api";
import { putPlannedDemoObject } from "./plannedDemoObjects";

const SCREEN_SERVER_ID = "00000000-0000-4005-8000-000000000001";
const LED_SERVER_ID = "00000000-0000-4005-8000-000000000002";
const SCREEN_SOURCE_ID = "00000000-0000-4005-8000-000000000011";
const LED_SOURCE_ID = "00000000-0000-4005-8000-000000000012";
const LED_MODULE_ID = "00000000-0000-4005-8000-000000000021";
const SCREEN_SURFACE_ID = "00000000-0000-4005-8000-000000000031";
const LED_SURFACE_ID = "00000000-0000-4005-8000-000000000032";

export async function installPlannedDemoMedia(api: ApiDriver, showId: string) {
	for (const kind of [
		"media_projector",
		"media_surface",
		"led_module_type",
		"media_source",
		"media_server",
	] as const) {
		for (const object of await api.showObjects<any>(showId, kind))
			await api.deleteSeededShowObject(
				showId,
				kind,
				object.id,
				object.revision,
			);
	}

	const servers = [
		{
			id: SCREEN_SERVER_ID,
			name: "Projection Media Server",
			citp: { host: "127.0.0.1", port: 4809 },
			lastKnownEndpoint: null,
			fixtureId: "00000000-0000-4001-8000-0000000003e9",
		},
		{
			id: LED_SERVER_ID,
			name: "LED Media Server",
			citp: { host: "127.0.0.1", port: 4810 },
			lastKnownEndpoint: null,
			fixtureId: "00000000-0000-4001-8000-0000000003ea",
		},
	];
	const sources = [
		{
			id: SCREEN_SOURCE_ID,
			serverId: SCREEN_SERVER_ID,
			advertisedSourceId: 1,
			name: "Projection Screens",
			outputName: "Screens Left + Right",
			width: 3840,
			height: 1080,
			aspectRatio: 32 / 9,
		},
		{
			id: LED_SOURCE_ID,
			serverId: LED_SERVER_ID,
			advertisedSourceId: 1,
			name: "Stage LED Panels",
			outputName: "LED Left + Center + Right",
			width: 5760,
			height: 1080,
			aspectRatio: 16 / 3,
		},
	];
	const ledModuleType = {
		id: LED_MODULE_ID,
		name: "Demo 500 mm LED Module",
		widthMetres: 0.5,
		heightMetres: 0.5,
		pixelPitchMillimetres: 3.9,
		horizontalGapMetres: 0.004,
		verticalGapMetres: 0.004,
		pixelWidth: 128,
		pixelHeight: 128,
	};
	const surfaces = [
		{
			id: SCREEN_SURFACE_ID,
			name: "Projection Screens",
			sourceId: SCREEN_SOURCE_ID,
			fallback: null,
			sections: [
				projectionSection(
					"00000000-0000-4005-8000-000000000041",
					"Projection Screen Left",
					-6,
				),
				projectionSection(
					"00000000-0000-4005-8000-000000000042",
					"Projection Screen Right",
					6,
				),
			],
		},
		{
			id: LED_SURFACE_ID,
			name: "Sunstrip LED Panels",
			sourceId: LED_SOURCE_ID,
			fallback: null,
			sections: [
				ledSection(
					"00000000-0000-4005-8000-000000000051",
					"LED Panel Left of Sunstrips",
					-3.25,
				),
				ledSection(
					"00000000-0000-4005-8000-000000000052",
					"LED Panel Between Sunstrips",
					0,
				),
				ledSection(
					"00000000-0000-4005-8000-000000000053",
					"LED Panel Right of Sunstrips",
					3.25,
				),
			],
		},
	];

	for (const server of servers)
		await putPlannedDemoObject(api, showId, "media_server", server.id, server);
	for (const source of sources)
		await putPlannedDemoObject(api, showId, "media_source", source.id, source);
	await putPlannedDemoObject(
		api,
		showId,
		"led_module_type",
		ledModuleType.id,
		ledModuleType,
	);
	for (const surface of surfaces)
		await putPlannedDemoObject(
			api,
			showId,
			"media_surface",
			surface.id,
			surface,
		);

	return { servers, sources, ledModuleType, surfaces };
}

function projectionSection(id: string, name: string, x: number) {
	return {
		id,
		name,
		transform: {
			positionMetres: [x, 2.6, -3.6],
			rotationDegrees: [0, 0, 0],
		},
		widthMetres: 4,
		heightMetres: 2.25,
		crop: { left: 0, top: 0, width: 1, height: 1 },
		type: "projection_screen",
		material: { type: "white" },
		edge_feather: 0.02,
	};
}

function ledSection(id: string, name: string, x: number) {
	return {
		id,
		name,
		transform: {
			positionMetres: [x, 2.5, -4.08],
			rotationDegrees: [0, 0, 0],
		},
		widthMetres: 2,
		heightMetres: 2,
		crop: { left: 0, top: 0, width: 1, height: 1 },
		type: "led",
		module_type_id: LED_MODULE_ID,
		rows: 4,
		columns: 4,
		occupied_cells: Array.from({ length: 16 }, (_, index) => index),
	};
}
