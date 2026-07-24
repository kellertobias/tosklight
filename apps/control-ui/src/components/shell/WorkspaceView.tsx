import { useApp } from "../../state/AppContext";
import { windowRegistry } from "../../windows/WindowRegistry";
import { DeskGrid } from "./DeskGrid";

export function WorkspaceView() {
  const { state } = useApp();
  if (state.builtIn) {
    const Window = windowRegistry[state.builtIn];
    return <main className="workspace-view built-in-view" data-light-surface="built-in" data-pane-type={state.builtIn} aria-label={`${state.builtIn} built-in`}><Window builtIn /></main>;
  }
  const desk = state.desks.find((item) => item.id === state.activeDeskId) ?? state.desks[0];
  return <main className="workspace-view" data-light-surface="desktop" data-desktop-id={desk.id} aria-label={`Desktop ${desk.name}`}><DeskGrid desk={desk} /></main>;
}
