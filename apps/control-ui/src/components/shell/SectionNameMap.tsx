import { useApp } from "../../state/AppContext";
import { Button } from "../common";

export function SectionNameMap() {
  const { state, dispatch } = useApp();
  if (!state.showSectionNames) return null;

  const programmer = state.controlMode === "programmer";
  const openDeskStatus = () => {
    dispatch({ type: "SET_MODAL", modal: "debugOpen", value: true });
  };

  return <div className="section-name-map" aria-label="Desk section names">
    <section className="section-name-region section-name-dock" aria-label="Dock section">
      <strong>Dock</strong>
    </section>
    <section className="section-name-region section-name-view" aria-label="View section">
      <strong>View</strong>
    </section>
    <section className="section-name-region section-name-command" aria-label="Command section">
      <strong className="section-name-command-title">Command Section</strong>
      <div className="section-name-command-line"><span>Command Line</span></div>
      <div className="section-name-command-left"><span>{programmer ? "Programmer" : "Playback"}</span></div>
      <div className="section-name-command-right"><span>{programmer ? "Num Block" : "Playback Speed Group Section"}</span></div>
    </section>
    <Button className="section-name-desk-status" onClick={openDeskStatus}><span aria-hidden="true">⌁</span> Desk Status</Button>
  </div>;
}
