import type {
	AppState,
	BuiltInWindow,
	FixtureSheetColumn,
	GridRect,
} from "../types";

export const clamp = (value: number, minimum: number, maximum: number) =>
	Math.max(minimum, Math.min(maximum, value));

export const normalizeFixtureSheetIncludedHeads = (
	value: unknown,
	legacyShowSubheads: unknown,
	legacyShowMasterHeads: unknown,
	fallback: AppState["fixtureSheetIncludedHeads"],
): AppState["fixtureSheetIncludedHeads"] => {
	if (
		value === "all" ||
		value === "no-sub-heads" ||
		value === "no-master-heads"
	)
		return value;
	if (legacyShowSubheads === false && legacyShowMasterHeads !== false)
		return "no-sub-heads";
	if (legacyShowMasterHeads === false && legacyShowSubheads !== false)
		return "no-master-heads";
	if (legacyShowSubheads === true || legacyShowMasterHeads === true)
		return "all";
	return fallback;
};
export const overlaps = (a: GridRect, b: GridRect) =>
	a.x < b.x + b.width &&
	a.x + a.width > b.x &&
	a.y < b.y + b.height &&
	a.y + a.height > b.y;
export const cueListWindowKind = (kind: BuiltInWindow): BuiltInWindow =>
	kind === "playback" || kind === "qlists"
		? "cuelists"
		: kind === "playback_pool" || kind === "qlist_pool"
			? "cuelist_pool"
			: kind === "cue_list" || kind === "qs"
				? "cues"
				: kind;
export const cueListWindowTitle = (title: string, kind: BuiltInWindow) => {
	if (kind === "cuelists") return "Cuelists";
	if (kind === "cuelist_pool") return "Cuelist Pool";
	if (kind !== "cues") return title;
	if (/^(cue list|sequence)$/i.test(title)) return "Cues · Cuelist";
	return title.replace(/^Qs\s*·\s*/i, "Cues · ").replace(/QList/g, "Cuelist");
};
export const fixtureSheetColumnIds = new Set<FixtureSheetColumn>([
	"id",
	"icon",
	"name",
	"patch",
	"intensity",
	"color",
	"position",
	"beam",
	"shapers",
	"focus",
	"control",
	"media",
]);
export const normalizeFixtureSheetColumns = (
	columns: readonly unknown[] | undefined,
	fallback: FixtureSheetColumn[],
	legacyShowPatch?: boolean,
) => {
	const normalized = columns
		?.map((column) => (column === "dimmer" ? "intensity" : column))
		.filter(
			(column, index): column is FixtureSheetColumn =>
				typeof column === "string" &&
				fixtureSheetColumnIds.has(column as FixtureSheetColumn) &&
				columns.findIndex(
					(candidate) =>
						(candidate === "dimmer" ? "intensity" : candidate) === column,
				) === index,
		);
	if (normalized?.length && legacyShowPatch && !normalized.includes("patch")) {
		const nameIndex = normalized.indexOf("name");
		normalized.splice(
			nameIndex < 0 ? normalized.length : nameIndex + 1,
			0,
			"patch",
		);
	}
	return normalized?.length ? normalized : fallback;
};

export const normalizeFixtureSheetCompactMode = (
	value: unknown,
): AppState["fixtureSheetCompactMode"] =>
	value === "icon-only" || value === "text-only" ? value : "off";
