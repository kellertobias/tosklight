import {
	createContext,
	type Dispatch,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useReducer,
} from "react";
import { useControlSurfaceTarget } from "../features/controlSurfaceInteraction/useControlSurfaceTarget";
import { DynamicEditorSessionProvider } from "../features/dynamics/DynamicEditorSessionContext";
import { frontendPerformanceDiagnostics } from "../features/frontendWarmup/diagnostics";
import type { AppState, BuiltInWindow } from "../types";
import { type Action, appReducer, initialState } from "./appReducer";

const shiftedWindows: Partial<Record<string, BuiltInWindow>> = {
	"1": "stage",
	"2": "fixtures",
	"3": "groups",
	"4": "presets",
	"5": "cuelists",
	"6": "channels",
	"7": "dmx",
	"8": "dynamics",
	"9": "help",
};

interface AppContextValue {
	state: AppState;
	dispatch: Dispatch<Action>;
}
const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: PropsWithChildren) {
	const [state, reduce] = useReducer(appReducer, initialState, (fallback) => {
		try {
			const saved = JSON.parse(
				localStorage.getItem("light.desk-controls") ?? "null",
			) as Partial<typeof fallback> | null;
			return saved
				? {
						...fallback,
						playbackColumns: saved.playbackColumns ?? fallback.playbackColumns,
						playbackRows: saved.playbackRows ?? fallback.playbackRows,
						regularNumberShortcuts:
							saved.regularNumberShortcuts ?? fallback.regularNumberShortcuts,
					}
				: fallback;
		} catch {
			return fallback;
		}
	});
	useEffect(() => {
		localStorage.setItem(
			"light.desk-controls",
			JSON.stringify({
				playbackColumns: state.playbackColumns,
				playbackRows: state.playbackRows,
				regularNumberShortcuts: state.regularNumberShortcuts,
			}),
		);
	}, [state.playbackColumns, state.playbackRows, state.regularNumberShortcuts]);
	useEffect(() => {
		document.documentElement.classList.toggle(
			"touch-scrollbars",
			state.touchScrollbars,
		);
		return () => document.documentElement.classList.remove("touch-scrollbars");
	}, [state.touchScrollbars]);
	const dispatch = useCallback<Dispatch<Action>>((action) => {
		const surface = measuredSurface(action);
		const finish = surface
			? frontendPerformanceDiagnostics.beginSurfaceSwitch(surface)
			: null;
		reduce(action);
		if (finish) afterNextPaint(finish);
	}, []);
	const setTarget = activeSetTarget(state);
	useControlSurfaceTarget(
		setTarget
			? {
					id: `app-set:${setTarget}`,
					priority: 100,
					accepts: ({ type }) => type === "set",
					handle: () => applySetTarget(setTarget, state, dispatch),
				}
			: null,
	);
	useControlSurfaceTarget({
		id: "app-desk-shortcuts",
		priority: 100,
		accepts: ({ type }) => type === "desk_shortcut",
		handle: (intent) => {
			if (intent.type !== "desk_shortcut") return;
			if (intent.action === "shift_down" || intent.action === "shift_up") {
				dispatch({
					type: "SET_SHIFT_ARMED",
					value: intent.action === "shift_down",
				});
				return;
			}
			if (intent.action === "shift_clear" || intent.action === "shift_delete") {
				dispatch({
					type: "SET_MODAL",
					modal: "systemControlsOpen",
					value: true,
				});
				return;
			}
			const kind = shiftedWindows[intent.action.slice(6)];
			if (kind) dispatch({ type: "OPEN_BUILTIN", kind });
		},
	});
	const value = useMemo(() => ({ state, dispatch }), [state]);
	return (
		<AppContext.Provider value={value}>
			<DynamicEditorSessionProvider>{children}</DynamicEditorSessionProvider>
		</AppContext.Provider>
	);
}

function activeSetTarget(state: AppState) {
	if (state.dockMode === "builtins" && state.builtIn === "patch")
		return "patch" as const;
	if (state.controlMode === "playbacks") return "playback" as const;
	const kinds =
		state.dockMode === "desks"
			? (state.desks
					.find((desk) => desk.id === state.activeDeskId)
					?.panes.filter(
						(pane) =>
							state.maximizedPaneId == null ||
							pane.id === state.maximizedPaneId,
					)
					.map((pane) => pane.kind) ?? [])
			: state.builtIn
				? [state.builtIn]
				: [];
	if (
		kinds.some((kind) =>
			[
				"cuelists",
				"cuelist_pool",
				"cue_list",
				"cues",
				"qlists",
				"qlist_pool",
				"qs",
			].includes(kind),
		)
	)
		return "cuelist" as const;
	if (
		kinds.some((kind) =>
			["playback", "playback_pool", "virtual_playbacks"].includes(kind),
		)
	)
		return "playback" as const;
	if (kinds.includes("presets")) return "preset" as const;
	return null;
}

function applySetTarget(
	target: NonNullable<ReturnType<typeof activeSetTarget>>,
	state: AppState,
	dispatch: Dispatch<Action>,
) {
	if (target === "patch")
		return dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed });
	if (target === "cuelist") {
		if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
		return dispatch({
			type: "SET_CUELIST_SET_ARMED",
			value: !state.cueListSetArmed,
		});
	}
	if (target === "playback")
		return dispatch({
			type: "SET_PLAYBACK_SET_ARMED",
			value: !state.playbackSetArmed,
		});
	dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed });
}

function measuredSurface(action: Action) {
	if (action.type === "OPEN_BUILTIN") return `built-in:${action.kind}`;
	if (action.type === "OPEN_DESK") return `desktop:${action.id}`;
	if (action.type === "TOGGLE_CONTROL_MODE") return "control-section";
	return null;
}

function afterNextPaint(finish: () => void) {
	if (typeof requestAnimationFrame !== "function")
		return queueMicrotask(finish);
	requestAnimationFrame(() => requestAnimationFrame(finish));
}

export function useApp() {
	const context = useContext(AppContext);
	if (!context) throw new Error("useApp must be used inside AppProvider");
	return context;
}

/** Optional access for reusable windows that are also rendered in isolation. */
export function useOptionalApp() {
	return useContext(AppContext);
}
