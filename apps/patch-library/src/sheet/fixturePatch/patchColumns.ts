/** Every patch sheet column, in the order the table draws them. The label is also the header. */
export const PATCH_SHEET_COLUMNS = [
	{ id: "type", label: "Type" },
	{ id: "fixture_id", label: "Fixture ID" },
	{ id: "name", label: "Name" },
	{ id: "manufacturer", label: "Manufacturer" },
	{ id: "mode", label: "Product / mode" },
	{ id: "patch", label: "Patch" },
	{ id: "masters", label: "Masters" },
	{ id: "invert_pan", label: "Invert Pan" },
	{ id: "invert_tilt", label: "Invert Tilt" },
	{ id: "mib", label: "MIB" },
	{ id: "location_x", label: "Location X" },
	{ id: "location_y", label: "Location Y" },
	{ id: "location_z", label: "Location Z" },
	{ id: "rotation_x", label: "Rotation X" },
	{ id: "rotation_y", label: "Rotation Y" },
	{ id: "rotation_z", label: "Rotation Z" },
	{ id: "bracket", label: "Bracket" },
	{ id: "shaper", label: "Shaper" },
	{ id: "footprint_width", label: "Footprint width" },
	{ id: "footprint_height", label: "Footprint height" },
	{ id: "footprint_depth", label: "Footprint depth" },
	{ id: "scenery_colour", label: "Colour" },
	{ id: "chain", label: "Chain" },
	{ id: "layer", label: "Layer" },
	{ id: "visible_2d", label: "2D" },
	{ id: "visible_3d", label: "3D" },
	{ id: "note", label: "Note" },
] as const;

export type PatchSheetColumn = (typeof PATCH_SHEET_COLUMNS)[number]["id"];

const LOCATION = ["location_x", "location_y", "location_z"] as const;
const ROTATION = ["rotation_x", "rotation_y", "rotation_z"] as const;

/** The Architect's quick views: each one shows exactly these columns, in table order. */
export const PATCH_QUICK_VIEWS = [
	{
		id: "patch",
		label: "Patch",
		columns: [
			"fixture_id",
			"name",
			"manufacturer",
			"mode",
			"patch",
			"masters",
			"invert_pan",
			"invert_tilt",
			"mib",
			"layer",
			"note",
		],
	},
	{
		id: "visualization",
		label: "Visualization",
		columns: [
			"fixture_id",
			"name",
			"layer",
			...LOCATION,
			...ROTATION,
			"bracket",
			"shaper",
			"footprint_width",
			"footprint_height",
			"footprint_depth",
			"scenery_colour",
			"chain",
			"visible_2d",
			"visible_3d",
			"note",
		],
	},
	{
		id: "compact",
		label: "Compact",
		columns: ["fixture_id", "name", "patch", "note", "layer"],
	},
] as const satisfies readonly {
	id: string;
	label: string;
	columns: readonly PatchSheetColumn[];
}[];

export type PatchQuickView = (typeof PATCH_QUICK_VIEWS)[number];

/** The hidden columns that leave exactly the view's columns on screen. */
export function quickViewHiddenColumns(view: PatchQuickView) {
	const shown = new Set<PatchSheetColumn>(view.columns);
	return PATCH_SHEET_COLUMNS.map(({ id }) => id).filter(
		(id) => !shown.has(id),
	);
}

/** The quick view the current columns match exactly, if any. */
export function activeQuickView(hidden: ReadonlySet<PatchSheetColumn>) {
	return PATCH_QUICK_VIEWS.find((view) => {
		const shown = new Set<PatchSheetColumn>(view.columns);
		return PATCH_SHEET_COLUMNS.every(
			({ id }) => shown.has(id) === !hidden.has(id),
		);
	});
}

/** Stored hidden columns, dropping anything this build no longer draws. */
export function parseHiddenColumns(stored: string | null): PatchSheetColumn[] {
	if (!stored) return [];
	try {
		const parsed: unknown = JSON.parse(stored);
		if (!Array.isArray(parsed)) return [];
		const known = new Set<string>(PATCH_SHEET_COLUMNS.map(({ id }) => id));
		const hidden = parsed.filter(
			(id): id is PatchSheetColumn => typeof id === "string" && known.has(id),
		);
		// A table with nothing to draw is never a valid stored state.
		return hidden.length >= PATCH_SHEET_COLUMNS.length ? [] : hidden;
	} catch {
		return [];
	}
}
