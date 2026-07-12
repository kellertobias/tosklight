import { createContext, useContext, useMemo, useReducer, type Dispatch, type PropsWithChildren } from "react";
import { appReducer, initialState, type Action } from "./appReducer";
import type { AppState } from "../types";

interface AppContextValue { state: AppState; dispatch: Dispatch<Action> }
const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: PropsWithChildren) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  const value = useMemo(() => ({ state, dispatch }), [state]);
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const context = useContext(AppContext);
  if (!context) throw new Error("useApp must be used inside AppProvider");
  return context;
}
