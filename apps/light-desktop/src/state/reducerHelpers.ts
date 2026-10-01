import type {
	AppState,
	BuiltInWindow,
	FixtureSheetColumn,
	GridRect,
	PatchColumn,
} from "../types";
import { PATCH_COLUMNS } from "../types";

export const clamp = (value: number, minimum: number, maximum: number) =>
	Math.max(minimum, Math.min(maximum, value));

function normalizePoolGridWidth(
	value: unknown,
	minimum: number,
	fallback: number,
): number {
	return typeof value === "number" && Number.isFinite(value)
		? clamp(Math.round(value), minimum, 320)
		: fallback;
}

export const normalizePoolGridDefaultWidth = (value: unknown, fallback: number) =>
	normalizePoolGridWidth(value, 56, fallback);

export const normalizePoolGridMinimumWidth = (value: unknown, fallback: number) =>
	normalizePoolGridWidth(value, 48, fallback);

type PoolGridSizing = Pick<AppState, "poolGridDefaultWidth" | "poolGridMinimumWidth">;

/**
 * Layouts saved before the default width existed stored the tile width the
 * pools laid out at as `poolGridMinimumWidth`; that value is now the default.
 */
export function normalizePoolGridSizing(
	saved: { poolGridDefaultWidth?: unknown; poolGridMinimumWidth?: unknown } | undefined,
	fallback: PoolGridSizing,
): PoolGridSizing {
	if (saved?.poolGridDefaultWidth === undefined)
		return {
			poolGridDefaultWidth: normalizePoolGridDefaultWidth(
				saved?.poolGridMinimumWidth,
				fallback.poolGridDefaultWidth,
			),
			poolGridMinimumWidth: fallback.poolGridMinimumWidth,
		};
	return {
		poolGridDefaultWidth: normalizePoolGridDefaultWidth(
			saved.poolGridDefaultWidth,
			fallback.poolGridDefaultWidth,
		),
		poolGridMinimumWidth: normalizePoolGridMinimumWidth(
			saved.poolGridMinimumWidth,
			fallback.poolGridMinimumWidth,
		),
	};
}

export const poolCardSizing = (state: PoolGridSizing) => ({
	defaultWidth: state.poolGridDefaultWidth,
	minimumWidth: state.poolGridMinimumWidth,
});

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

/**
 * Hidden Show Patch columns from a stored layout: unknown and repeated ids are dropped, and a list
 * that would hide every column hides none, so the table can never be left empty.
 */
export const normalizePatchHiddenColumns = (value: unknown): PatchColumn[] => {
	if (!Array.isArray(value)) return [];
	const hidden = PATCH_COLUMNS.map((column) => column.id).filter((id) =>
		value.includes(id),
	);
	return hidden.length < PATCH_COLUMNS.length ? hidden : [];
};

export const normalizeFixtureSheetCompactMode = (
	value: unknown,
): AppState["fixtureSheetCompactMode"] =>
	value === "icon-only" || value === "text-only" ? value : "off";
