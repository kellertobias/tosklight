import type { AppState, FixtureSheetCompactMode } from "../../types";

const STORAGE_PREFIX = "tosklight.fixture-sheet-compact-modes.v1";

export interface FixtureSheetCompactModes {
	builtIn: FixtureSheetCompactMode;
	desktops: Record<string, Record<string, FixtureSheetCompactMode>>;
}

function normalizeMode(value: unknown): FixtureSheetCompactMode {
	return value === "icon-only" || value === "text-only" ? value : "off";
}

export function fixtureSheetCompactModeStorageKey(
	showId: string,
	controlDeskId: string,
): string {
	return `${STORAGE_PREFIX}:${showId}:${controlDeskId}`;
}

export function readFixtureSheetCompactModes(
	showId: string,
	controlDeskId: string,
): FixtureSheetCompactModes {
	try {
		const parsed = JSON.parse(
			localStorage.getItem(
				fixtureSheetCompactModeStorageKey(showId, controlDeskId),
			) ?? "null",
		) as Partial<FixtureSheetCompactModes> | null;
		const desktops: FixtureSheetCompactModes["desktops"] = {};
		if (parsed?.desktops && typeof parsed.desktops === "object") {
			for (const [desktopId, panes] of Object.entries(parsed.desktops)) {
				if (!panes || typeof panes !== "object") continue;
				desktops[desktopId] = Object.fromEntries(
					Object.entries(panes).map(([paneId, mode]) => [
						paneId,
						normalizeMode(mode),
					]),
				);
			}
		}
		return { builtIn: normalizeMode(parsed?.builtIn), desktops };
	} catch {
		return { builtIn: "off", desktops: {} };
	}
}

export function collectFixtureSheetCompactModes(
	state: Pick<AppState, "fixtureSheetCompactMode" | "desks">,
): FixtureSheetCompactModes {
	return {
		builtIn: state.fixtureSheetCompactMode,
		desktops: Object.fromEntries(
			state.desks
				.map(
					(desktop) =>
						[
							desktop.id,
							Object.fromEntries(
								desktop.panes
									.filter((pane) => pane.kind === "fixtures")
									.map((pane) => [
										pane.id,
										pane.fixtureSheetCompactMode ?? "off",
									]),
							),
						] as const,
				)
				.filter(([, panes]) => Object.keys(panes).length > 0),
		),
	};
}

export function desksWithoutFixtureSheetCompactModes(
	desks: AppState["desks"],
): AppState["desks"] {
	return desks.map((desktop) => ({
		...desktop,
		panes: desktop.panes.map((pane) => {
			const { fixtureSheetCompactMode: _compactMode, ...portablePane } = pane;
			return portablePane;
		}),
	}));
}
