import { normalizePresetFamily } from "../../presetFamilies";
import type { AppState } from "../../types";
import { normalizeVisualizationRows } from "../../windows/visualizationPaneModel";
import type { Action } from "../appActions";
import {
	cueListWindowKind,
	cueListWindowTitle,
	normalizeFixtureSheetColumns,
	normalizeFixtureSheetCompactMode,
	normalizeFixtureSheetIncludedHeads,
} from "../reducerHelpers";

/**
 * Which side a saved layout is looking at the rig from.
 *
 * A layout from before the renderer drew every Stage carries a render style rather than a side.
 * There is nothing in one to convert — a style said how much of a beam to draw, not where to
 * stand — so it is dropped and the Stage opens on the plan, which is what a 2D Stage was.
 */
/** A saved render quality, or the current one for anything this build does not know. */
function normalizeStageVizQuality(
	value: unknown,
	fallback: AppState["stageVizQuality"],
): AppState["stageVizQuality"] {
	return value === "draft" ||
		value === "standard" ||
		value === "high" ||
		value === "ultra"
		? value
		: fallback;
}

/** A saved number, or the current one where the layout carries something else. */
function numberOr(value: unknown, fallback: number): number {
	return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function normalizeStage2dSide(value: unknown): AppState["stage2dSide"] {
	return value === "top" ||
		value === "front" ||
		value === "back" ||
		value === "left" ||
		value === "right"
		? value
		: "top";
}

function normalizeChannelDisplayMode(
	value: unknown,
): NonNullable<
	AppState["desks"][number]["panes"][number]["channelDisplayMode"]
> {
	return value === "all" ? "all" : "intensity";
}

export function nextDesktopId(desks: readonly { id: string }[]): string {
	let suffix = desks.length + 1;
	while (desks.some((desk) => desk.id === `desk-${suffix}`)) suffix += 1;
	return `desk-${suffix}`;
}

function isRetiredWindow(kind: unknown): boolean {
	return kind === "development" || kind === "layout";
}

function schedulerPaneLayout(pane: AppState["desks"][number]["panes"][number]) {
	const requestedList = pane.schedulerShowList !== false;
	const requestedCalendar = pane.schedulerShowCalendar !== false;
	return requestedList || requestedCalendar
		? {
				schedulerShowList: requestedList,
				schedulerShowCalendar: requestedCalendar,
			}
		: {
				schedulerShowList: true,
				schedulerShowCalendar: false,
			};
}

function hydrateDesks(
	state: AppState,
	action: Extract<Action, { type: "HYDRATE_LAYOUT" }>,
): AppState["desks"] {
	return action.desks.map((desk) => ({
		...desk,
		panes: desk.panes
			.filter((pane) => !isRetiredWindow(pane.kind))
			.map((pane) => {
				const { layoutGroupId: _retiredPaneGroupId, ...activePane } = pane;
				const kind = cueListWindowKind(pane.kind);
				const migrated = {
					...activePane,
					kind,
					title: cueListWindowTitle(pane.title, kind),
					...(kind === "scheduler" ? schedulerPaneLayout(pane) : {}),
					...(kind === "channels"
						? {
								channelDisplayMode: normalizeChannelDisplayMode(
									pane.channelDisplayMode,
								),
							}
						: {}),
					...(kind === "visualization"
						? {
								visualizationRows: normalizeVisualizationRows(
									pane.visualizationRows,
								),
							}
						: {}),
					...(kind === "fixtures"
						? { fixtureSheetCompactMode: "off" as const }
						: {}),
				};
				if (pane.kind !== "presets") return migrated;
				const legacyDefault =
					pane.title === "All Presets" ||
					(pane.id === "presets" && pane.title === "Color & Position Presets");
				return {
					...migrated,
					title: legacyDefault ? "Mixed Presets" : pane.title,
					presetFamily: legacyDefault
						? "Mixed"
						: normalizePresetFamily(
								pane.presetFamily,
								normalizePresetFamily(
									action.windowSettings?.presetFamily,
									state.presetFamily,
								),
							),
				};
			}),
	}));
}

export function reduceHydration(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "HYDRATE_LAYOUT": {
			const retiredLayout =
				action.windowSettings?.builtIn === "layout" ||
				action.windowSettings?.lastBuiltIn === "layout" ||
				action.windowSettings?.layoutGroupId != null ||
				action.desks.some((desk) =>
					desk.panes.some(
						(pane) => pane.kind === "layout" || pane.layoutGroupId != null,
					),
				);
			const { layoutGroupId: _retiredLayoutGroupId, ...windowSettings } =
				action.windowSettings ?? {};
			return {
				...state,
				...windowSettings,
				layoutMigrationNotice: retiredLayout,
				fixtureSheetIncludedHeads: normalizeFixtureSheetIncludedHeads(
					action.windowSettings?.fixtureSheetIncludedHeads,
					action.windowSettings?.fixtureSheetShowSubheads,
					action.windowSettings?.fixtureSheetShowMasterHeads,
					state.fixtureSheetIncludedHeads,
				),
				fixtureSheetColumns: normalizeFixtureSheetColumns(
					action.windowSettings?.fixtureSheetColumns,
					state.fixtureSheetColumns,
					action.windowSettings?.fixtureSheetShowPatch,
				),
				// Compact modes are installation-local and hydrate separately by real desk.
				fixtureSheetCompactMode: "off",
				presetFamily: normalizePresetFamily(
					action.windowSettings?.presetFamily,
					state.presetFamily,
				),
				// Persisted layouts predating the Setup-positions removal may still carry it.
				stageMode:
					action.windowSettings?.stageMode === "select" ||
					action.windowSettings?.stageMode === "navigate"
						? action.windowSettings.stageMode
						: state.stageMode,
				stage2dSide: normalizeStage2dSide(action.windowSettings?.stage2dSide),
				stageVizBackground:
					typeof action.windowSettings?.stageVizBackground === "string"
						? action.windowSettings.stageVizBackground
						: state.stageVizBackground,
				stageVizQuality: normalizeStageVizQuality(
					action.windowSettings?.stageVizQuality,
					state.stageVizQuality,
				),
				stageVizAtmosphere: numberOr(
					action.windowSettings?.stageVizAtmosphere,
					state.stageVizAtmosphere,
				),
				stageVizExposure: numberOr(
					action.windowSettings?.stageVizExposure,
					state.stageVizExposure,
				),
				stageVizLaserBrightness: numberOr(
					action.windowSettings?.stageVizLaserBrightness,
					state.stageVizLaserBrightness,
				),
				stageVizShowLabels:
					typeof action.windowSettings?.stageVizShowLabels === "boolean"
						? action.windowSettings.stageVizShowLabels
						: state.stageVizShowLabels,
				builtIn:
					action.windowSettings?.builtIn == null
						? (action.windowSettings?.builtIn ?? state.builtIn)
						: isRetiredWindow(action.windowSettings.builtIn)
							? null
							: cueListWindowKind(action.windowSettings.builtIn),
				lastBuiltIn: isRetiredWindow(action.windowSettings?.lastBuiltIn)
					? state.lastBuiltIn
					: cueListWindowKind(
							action.windowSettings?.lastBuiltIn ?? state.lastBuiltIn,
						),
				desks: hydrateDesks(state, action),
				activeDeskId: action.desks.some(
					(desk) => desk.id === action.activeDeskId,
				)
					? action.activeDeskId
					: (action.desks[0]?.id ?? state.activeDeskId),
				savingDesk: false,
			};
		}
		case "HYDRATE_FIXTURE_SHEET_COMPACT_MODES":
			return {
				...state,
				fixtureSheetCompactMode: normalizeFixtureSheetCompactMode(
					action.builtIn,
				),
				desks: state.desks.map((desktop) => ({
					...desktop,
					panes: desktop.panes.map((pane) =>
						pane.kind === "fixtures"
							? {
									...pane,
									fixtureSheetCompactMode: normalizeFixtureSheetCompactMode(
										action.desktops[desktop.id]?.[pane.id],
									),
								}
							: pane,
					),
				})),
			};
		case "DISMISS_LAYOUT_MIGRATION_NOTICE":
			return { ...state, layoutMigrationNotice: false };
		case "NEW_DESK": {
			const id = nextDesktopId(state.desks);
			const source = state.desks.find((desk) => desk.id === state.activeDeskId);
			const panes =
				state.savingDesk && source
					? source.panes.map((pane, index) => ({
							...pane,
							id: `${id}-${pane.kind}-${index + 1}`,
						}))
					: [];
			return {
				...state,
				desks: [
					...state.desks,
					{ id, name: `Desktop ${state.desks.length + 1}`, panes },
				],
				activeDeskId: id,
				builtIn: null,
				savingDesk: false,
			};
		}
		default:
			return undefined;
	}
}
