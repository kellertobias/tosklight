import { normalizePresetFamily } from "../../presetFamilies";
import type { AppState } from "../../types";
import type { Action } from "../appActions";
import {
	cueListWindowKind,
	cueListWindowTitle,
	normalizeFixtureSheetColumns,
	normalizeFixtureSheetIncludedHeads,
} from "../reducerHelpers";

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
				builtIn:
					action.windowSettings?.builtIn == null
						? (action.windowSettings?.builtIn ?? state.builtIn)
						: cueListWindowKind(action.windowSettings.builtIn),
				lastBuiltIn: cueListWindowKind(
					action.windowSettings?.lastBuiltIn ?? state.lastBuiltIn,
				),
				desks: action.desks.map((desk) => ({
					...desk,
					panes: desk.panes.map((pane) => {
						const kind = cueListWindowKind(pane.kind);
						const migrated = {
							...pane,
							kind,
							title: cueListWindowTitle(pane.title, kind),
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
			const id = `desk-${state.desks.length + 1}`;
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
