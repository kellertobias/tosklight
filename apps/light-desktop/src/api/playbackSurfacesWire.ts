import type * as Wire from "./generated/light-wire";
import type {
	PlaybackPageSource,
	PlaybackSurfaceConfiguration,
	PlaybackSurfacesSnapshot,
} from "./playbackConfigurationModels";
import {
	arrayAt,
	boundedPositiveIntegerAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";
export function decodePlaybackSurfaces(
	value: unknown,
): PlaybackSurfacesSnapshot {
	const root = exactRecordAt(value, "$", [
		"playback_surfaces",
		"selected_surface_id",
		"resolved_pages",
		"command_page",
	]);
	const registry = recordAt(root.playback_surfaces, "$.playback_surfaces");
	const playback_surfaces = Object.fromEntries(
		Object.entries(registry).map(([id, v]) => [
			id,
			configuration(v, `$.playback_surfaces.${id}`),
		]),
	);
	const resolved = recordAt(root.resolved_pages, "$.resolved_pages");
	const resolved_pages = Object.fromEntries(
		Object.entries(resolved).map(([id, page]) => [
			id,
			boundedPositiveIntegerAt(page, `$.resolved_pages.${id}`, 99),
		]),
	);
	const selected_surface_id = stringAt(
		root.selected_surface_id,
		"$.selected_surface_id",
	);
	const command_page = boundedPositiveIntegerAt(
		root.command_page,
		"$.command_page",
		99,
	);
	if (
		!playback_surfaces[selected_surface_id] ||
		resolved_pages[selected_surface_id] !== command_page
	)
		throw new WireValidationError(
			"$.command_page",
			"selected surface resolved page",
			command_page,
		);
	return {
		playback_surfaces,
		resolved_pages,
		selected_surface_id,
		command_page,
	} satisfies Wire.PlaybackSurfacesSnapshot;
}
function configuration(
	value: unknown,
	path: string,
): PlaybackSurfaceConfiguration {
	const surface = exactRecordAt(value, path, ["name", "layout", "page_source"]);
	const layout = exactRecordAt(surface.layout, `${path}.layout`, ["rows"]);
	const rows = arrayAt(layout.rows, `${path}.layout.rows`).map(
		(value, index) => {
			const p = `${path}.layout.rows[${index}]`;
			const row = exactRecordAt(value, p, [
				"first_position",
				"count",
				"presentation",
				"button_count",
			]);
			const first_position = boundedPositiveIntegerAt(
				row.first_position,
				`${p}.first_position`,
				1000,
			);
			const count = boundedPositiveIntegerAt(row.count, `${p}.count`, 1000);
			if (first_position + count - 1 > 1000)
				throw new WireValidationError(p, "row positions within 1..1000", row);
			const button_count = integerAt(row.button_count, `${p}.button_count`);
			if (button_count > 3)
				throw new WireValidationError(
					`${p}.button_count`,
					"0..3",
					button_count,
				);
			return {
				first_position,
				count,
				presentation: enumAt(row.presentation, `${p}.presentation`, [
					"fader",
					"buttons",
					"push_encoder",
				]),
				button_count,
			};
		},
	);
	return {
		name: stringAt(surface.name, `${path}.name`),
		layout: { rows },
		page_source: pageSource(surface.page_source, `${path}.page_source`),
	};
}
function pageSource(value: unknown, path: string): PlaybackPageSource {
	const v = recordAt(value, path);
	const type = enumAt(v.type, `${path}.type`, ["pinned", "own", "follow"]);
	if (type === "follow") {
		exactRecordAt(v, path, ["type", "surface_id"]);
		return { type, surface_id: stringAt(v.surface_id, `${path}.surface_id`) };
	}
	exactRecordAt(v, path, ["type", "page"]);
	return { type, page: boundedPositiveIntegerAt(v.page, `${path}.page`, 99) };
}
