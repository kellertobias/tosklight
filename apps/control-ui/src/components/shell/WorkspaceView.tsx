import { useApp } from "../../state/AppContext";
import { windowRegistry } from "../../windows/WindowRegistry";
import { DeskGrid } from "./DeskGrid";

export function WorkspaceView() {
  const { state } = useApp();
  if (state.builtIn) {
    const Window = windowRegistry[state.builtIn];
    return <main className="workspace-view built-in-view"><Window builtIn /></main>;
  }
  const desk = state.desks.find((item) => item.id === state.activeDeskId) ?? state.desks[0];
  return <main className="workspace-view"><DeskGrid desk={desk} /></main>;
}
