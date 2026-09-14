import { describe, expect, it } from "vitest";
import {
	activeQuickView,
	PATCH_QUICK_VIEWS,
	PATCH_SHEET_COLUMNS,
	parseHiddenColumns,
	quickViewHiddenColumns,
} from "./patchColumns";

const label = (id: string) =>
	PATCH_SHEET_COLUMNS.find((column) => column.id === id)?.label;

function shownLabels(viewId: string) {
	const view = PATCH_QUICK_VIEWS.find((candidate) => candidate.id === viewId);
	if (!view) throw new Error(`No ${viewId} view`);
	const hidden = new Set(quickViewHiddenColumns(view));
	return PATCH_SHEET_COLUMNS.filter(({ id }) => !hidden.has(id)).map(({ id }) =>
		label(id),
	);
}

describe("patch sheet quick views", () => {
	it("show exactly the columns each view names, in table order", () => {
		expect(shownLabels("patch")).toEqual([
			"Fixture ID",
			"Name",
			"Manufacturer",
			"Product / mode",
			"Patch",
			"Group Masters",
			"Grand Master",
			"MIB",
			"MIB Delay",
			"Layer",
			"Note",
		]);
		expect(shownLabels("visualization")).toEqual([
			"Fixture ID",
			"Name",
			"Location X",
			"Location Y",
			"Location Z",
			"Rotation X",
			"Rotation Y",
			"Rotation Z",
			"Bracket",
			"Shaper",
			"Layer",
			"2D",
			"3D",
			"Note",
		]);
		expect(shownLabels("compact")).toEqual([
			"Fixture ID",
			"Name",
			"Patch",
			"Layer",
			"Note",
		]);
	});

	it("marks a view active only while the columns match it exactly", () => {
		const compact = PATCH_QUICK_VIEWS[2];
		const hidden = new Set(quickViewHiddenColumns(compact));
		expect(activeQuickView(hidden)?.id).toBe("compact");
		hidden.delete("type");
		expect(activeQuickView(hidden)).toBeUndefined();
		expect(activeQuickView(new Set())).toBeUndefined();
	});

	it("reads stored columns defensively", () => {
		expect(parseHiddenColumns(null)).toEqual([]);
		expect(parseHiddenColumns("not json")).toEqual([]);
		expect(parseHiddenColumns('["type","retired","note"]')).toEqual([
			"type",
			"note",
		]);
		expect(
			parseHiddenColumns(JSON.stringify(PATCH_SHEET_COLUMNS.map(({ id }) => id))),
		).toEqual([]);
	});
});
