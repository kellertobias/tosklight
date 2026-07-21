import type { AppState } from "../../types";
import type { Action } from "../appActions";
import { clamp } from "../reducerHelpers";

export function reduceControls(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_SPEED_GROUP":
			return { ...state, speedGroup: action.value };
		case "SET_PLAYBACK_LAYOUT":
			return {
				...state,
				playbackColumns: clamp(action.columns, 1, 32),
				playbackRows: clamp(action.rows, 1, 3),
			};
		case "SET_PLAYBACK_PAGE":
			return {
				...state,
				playbackPage: clamp(action.page, 0, state.playbackPageNames.length - 1),
			};
		case "SET_PRESET_FAMILY":
			return { ...state, presetFamily: action.family };
		case "SET_PRESET_POOL_COLORS":
			return { ...state, presetPoolColors: action.value };
		case "SET_PRESET_SET_ARMED":
			return { ...state, presetSetArmed: action.value };
		case "OPEN_BUILTIN_CUELIST":
			return {
				...state,
				cuelistBuiltInView: "cues",
				cuelistBuiltInNumber: action.number,
			};
		case "SET_BUILTIN_CUELIST_VIEW":
			return { ...state, cuelistBuiltInView: action.value };
		case "SET_CUELIST_SET_ARMED":
			return {
				...state,
				cueListSetArmed: action.value,
				cueListSetTarget: action.value ? state.cueListSetTarget : null,
			};
		case "SET_CUELIST_SET_TARGET":
			return {
				...state,
				cueListSetArmed: action.value != null,
				cueListSetTarget: action.value,
			};
		case "SET_PLAYBACK_SET_ARMED":
			return { ...state, playbackSetArmed: action.value };
		case "SET_MODAL":
			return { ...state, [action.modal]: action.value };
		case "OPEN_SPECIAL_DIALOG":
			return {
				...state,
				specialDialogFamily: action.family,
				specialDialogsOpen: true,
			};
		case "TOGGLE_MIDI_PROFILE":
			return { ...state, midiProfile: !state.midiProfile };
		case "SET_MIDI_PROFILE":
			return { ...state, midiProfile: action.value };
		case "TOGGLE_TOUCH_SCROLLBARS":
			return { ...state, touchScrollbars: !state.touchScrollbars };
		case "TOGGLE_SECTION_NAMES":
			return { ...state, showSectionNames: !state.showSectionNames };
		case "SET_REGULAR_NUMBER_SHORTCUTS":
			return { ...state, regularNumberShortcuts: action.value };
		case "SET_TEXT_EDITOR_FILE":
			return {
				...state,
				desks: state.desks.map((desk) => ({
					...desk,
					panes: desk.panes.map((pane) =>
						pane.id === action.id
							? {
									...pane,
									textFileRoot: action.root,
									textFilePath: action.path,
								}
							: pane,
					),
				})),
			};
		case "SET_TEXT_EDITOR_SETTINGS":
			return {
				...state,
				desks: state.desks.map((desk) => ({
					...desk,
					panes: desk.panes.map((pane) =>
						pane.id === action.id
							? {
									...pane,
									textEditorReadOnly:
										action.readOnly ?? pane.textEditorReadOnly ?? false,
									textEditorMode: action.mode ?? pane.textEditorMode ?? "plain",
								}
							: pane,
					),
				})),
			};
		case "SET_TEXT_EDITOR_VIEW":
			return {
				...state,
				desks: state.desks.map((desk) => ({
					...desk,
					panes: desk.panes.map((pane) =>
						pane.id === action.id
							? {
									...pane,
									textEditorView: {
										root: action.root,
										path: action.path,
										selectionStart: action.selectionStart,
										selectionEnd: action.selectionEnd,
										scrollTop: action.scrollTop,
									},
								}
							: pane,
					),
				})),
			};
		case "SET_STORE_ARMED":
			return {
				...state,
				storeArmed: action.value,
				updateArmed: action.value ? false : state.updateArmed,
			};
		case "SET_UPDATE_ARMED":
			return {
				...state,
				updateArmed: action.value,
				storeArmed: action.value ? false : state.storeArmed,
			};
		case "SET_SHIFT_ARMED":
			return { ...state, shiftArmed: action.value };
		case "SET_PATCH_ARMED":
			return { ...state, patchSetArmed: action.value };
		default:
			return undefined;
	}
}
