import { normalizePresetFamily } from "../../presetFamilies";
import type { AppState } from "../../types";
import type { Action } from "../appActions";
import {
	cueListWindowKind,
	cueListWindowTitle,
	normalizeFixtureSheetColumns,
	normalizeFixtureSheetIncludedHeads,
} from "../reducerHelpers";

function normalizeStageRenderQuality(
	value: unknown,
): AppState["stageRenderQuality"] {
	return value === "lines_only" ||
		value === "lines_and_beams" ||
		value === "beams" ||
		value === "improved_beams"
		? value
		: "lines_and_beams";
}

export function nextDesktopId(desks: readonly { id: string }[]): string {
	let suffix = desks.length + 1;
	while (desks.some((desk) => desk.id === `desk-${suffix}`)) suffix += 1;
	return `desk-${suffix}`;
}

function isRetiredDevelopmentWindow(kind: unknown): boolean {
	return kind === "development";
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

export function reduceHydration(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "HYDRATE_LAYOUT":
			return {
				...state,
				...action.windowSettings,
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
				stageRenderQuality: normalizeStageRenderQuality(
					action.windowSettings?.stageRenderQuality,
				),
				builtIn:
					action.windowSettings?.builtIn == null
						? (action.windowSettings?.builtIn ?? state.builtIn)
						: isRetiredDevelopmentWindow(action.windowSettings.builtIn)
							? null
							: cueListWindowKind(action.windowSettings.builtIn),
				lastBuiltIn: isRetiredDevelopmentWindow(
					action.windowSettings?.lastBuiltIn,
				)
					? state.lastBuiltIn
					: cueListWindowKind(
							action.windowSettings?.lastBuiltIn ?? state.lastBuiltIn,
						),
				desks: action.desks.map((desk) => ({
					...desk,
					panes: desk.panes
						.filter((pane) => !isRetiredDevelopmentWindow(pane.kind))
						.map((pane) => {
							const kind = cueListWindowKind(pane.kind);
							const migrated = {
								...pane,
								kind,
								title: cueListWindowTitle(pane.title, kind),
								...(kind === "stage"
									? {
											stageRenderQuality: normalizeStageRenderQuality(
												pane.stageRenderQuality,
											),
										}
									: {}),
								...(kind === "scheduler" ? schedulerPaneLayout(pane) : {}),
							};
							if (pane.kind !== "presets") return migrated;
							const legacyDefault =
								pane.title === "All Presets" ||
								(pane.id === "presets" &&
									pane.title === "Color & Position Presets");
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
				})),
				activeDeskId: action.desks.some(
					(desk) => desk.id === action.activeDeskId,
				)
					? action.activeDeskId
					: (action.desks[0]?.id ?? state.activeDeskId),
				savingDesk: false,
			};
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
