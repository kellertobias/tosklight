import { createContext, useContext, useEffect, useMemo, useReducer, type Dispatch, type PropsWithChildren } from "react";
import { appReducer, initialState, type Action } from "./appReducer";
import type { AppState } from "../types";
import type { BuiltInWindow } from "../types";

const shiftedWindows: Partial<Record<string, BuiltInWindow>> = { "1":"stage", "2":"fixtures", "3":"groups", "4":"presets", "5":"cuelists", "6":"channels", "7":"dmx", "8":"dynamics", "9":"help" };

interface AppContextValue { state: AppState; dispatch: Dispatch<Action> }
const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: PropsWithChildren) {
  const [state, dispatch] = useReducer(appReducer, initialState, (fallback) => {
    try {
      const saved = JSON.parse(localStorage.getItem("light.desk-controls") ?? "null") as Partial<typeof fallback> | null;
      return saved ? { ...fallback, playbackColumns: saved.playbackColumns ?? fallback.playbackColumns, playbackRows: saved.playbackRows ?? fallback.playbackRows, regularNumberShortcuts: saved.regularNumberShortcuts ?? fallback.regularNumberShortcuts } : fallback;
    } catch { return fallback; }
  });
  useEffect(() => {
    localStorage.setItem("light.desk-controls", JSON.stringify({ playbackColumns: state.playbackColumns, playbackRows: state.playbackRows, regularNumberShortcuts: state.regularNumberShortcuts }));
  }, [state.playbackColumns, state.playbackRows, state.regularNumberShortcuts]);
  useEffect(() => {
    document.documentElement.classList.toggle("touch-scrollbars", state.touchScrollbars);
    return () => document.documentElement.classList.remove("touch-scrollbars");
  }, [state.touchScrollbars]);
  useEffect(() => {
    const deskAction = (event: Event) => { const action = (event as CustomEvent<string>).detail; if (action === "shift-down" || action === "shift-up") { dispatch({ type: "SET_SHIFT_ARMED", value: action === "shift-down" }); return; } if (action === "shift-clear" || action === "shift-delete" || action === "shift-del") { dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true }); return; } if (action.startsWith("shift-")) { const kind = shiftedWindows[action.slice(6)]; if (kind) dispatch({ type: "OPEN_BUILTIN", kind }); return; } if (action !== "set") return; if (state.builtIn === "patch") dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed }); else if (document.querySelector(".cuelist-window.pool-window")) { if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false }); dispatch({ type: "SET_CUELIST_SET_ARMED", value: !state.cueListSetArmed }); } else if (document.querySelector(".playback-fader-bank,.virtual-playback-grid")) dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: !state.playbackSetArmed }); else if (state.builtIn === "presets" || state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "presets")) dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed }); };
    window.addEventListener("light:desk-action", deskAction);
    return () => window.removeEventListener("light:desk-action", deskAction);
  }, [state.builtIn, state.patchSetArmed, state.presetSetArmed, state.cueListSetArmed, state.playbackSetArmed, state.storeArmed, state.desks, state.activeDeskId]);
  const value = useMemo(() => ({ state, dispatch }), [state]);
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
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
