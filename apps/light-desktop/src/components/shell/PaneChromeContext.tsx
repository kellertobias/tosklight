import { createContext, useContext, type ReactNode } from "react";

export interface PaneChromeTargets {
  info: HTMLElement | null;
  toolbar: HTMLElement | null;
}

const PaneChromeContext = createContext<PaneChromeTargets | null>(null);

export function PaneChromeProvider({ value, children }: { value: PaneChromeTargets; children: ReactNode }) {
  return <PaneChromeContext.Provider value={value}>{children}</PaneChromeContext.Provider>;
}

/**
 * Pane windows use these portal targets to place their live state and actions
 * in the pane's one authoritative title bar. Standalone renders return null
 * and keep their local fallback toolbar.
 */
export function usePaneChromeTargets() {
  return useContext(PaneChromeContext);
}
