import { createContext, useContext, useEffect, useMemo, useReducer, type Dispatch, type PropsWithChildren } from "react";
import { appReducer, initialState, type Action } from "./appReducer";
import type { AppState } from "../types";

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
    const deskAction = (event: Event) => { if ((event as CustomEvent<string>).detail !== "set") return; if (state.builtIn === "patch") dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed }); else if (state.builtIn === "presets" || state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "presets")) dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed }); };
    window.addEventListener("light:desk-action", deskAction);
    return () => window.removeEventListener("light:desk-action", deskAction);
  }, [state.builtIn, state.patchSetArmed, state.presetSetArmed, state.desks, state.activeDeskId]);
  const value = useMemo(() => ({ state, dispatch }), [state]);
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const context = useContext(AppContext);
  if (!context) throw new Error("useApp must be used inside AppProvider");
  return context;
}
