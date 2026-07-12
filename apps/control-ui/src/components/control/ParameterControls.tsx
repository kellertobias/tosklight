import { useState, type CSSProperties } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";

const parameterFamilies = {
  Intensity: [["Dimmer", "75%", 75, "intensity"], ["Shutter", "Open", 100, "shutter"], ["Strobe", "0 Hz", 0, "strobe"], ["Master", "100%", 100, "master"]],
  Color: [["Red", "120", 47, "color.red"], ["Green", "138", 54, "color.green"], ["Blue", "220", 86, "color.blue"], ["White", "0", 0, "color.white"]],
  Position: [["Pan", "71°", 61, "pan"], ["Tilt", "36°", 43, "tilt"], ["Pan fine", "0", 0, "pan.fine"], ["Tilt fine", "0", 0, "tilt.fine"]],
  Beam: [["Gobo", "Open", 0, "gobo"], ["Prism", "Open", 0, "prism"], ["Iris", "100%", 100, "iris"], ["Frost", "0%", 0, "frost"]],
  Focus: [["Focus", "50%", 50, "focus"], ["Zoom", "35%", 35, "zoom"], ["Frost", "0%", 0, "frost"], ["Edge", "Sharp", 10, "edge"]],
  Control: [["Reset", "Safe", 0, "control.reset"], ["Lamp", "On", 100, "control.lamp"], ["Fan", "Auto", 50, "control.fan"], ["Mode", "Normal", 0, "control.mode"]],
  Media: [["Layer", "1", 10, "media.layer"], ["Clip", "1", 10, "media.clip"], ["Opacity", "100%", 100, "media.opacity"], ["Speed", "100%", 50, "media.speed"]],
  Dynamics: [["Speed", "30 BPM", 25, "dynamic.speed"], ["Phase", "0°", 0, "dynamic.phase"], ["Width", "50%", 50, "dynamic.width"], ["Blocks", "2", 20, "dynamic.blocks"]],
} as const;

type Family = keyof typeof parameterFamilies;
const specialFamilies = new Set<Family>(["Color", "Position", "Beam", "Control", "Dynamics"]);

export function ParameterControls() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [family, setFamily] = useState<Family>("Intensity");
  const applyParameter = async (attribute: string, level: number) => {
    const fixtureIds = server.selectedFixtures.length ? server.selectedFixtures : server.patch?.fixtures.slice(0, 8).map((fixture) => fixture.fixture_id) ?? [];
    if (server.selectedGroupId) { await (state.preload === "idle" ? server.setGroupValue(attribute, level / 100) : server.setPreloadGroupValue(attribute, level / 100)); return; }
    if (!fixtureIds.length) return;
    await server.setSelection(fixtureIds);
    await Promise.all(fixtureIds.map((fixtureId) => server.setProgrammer(fixtureId, attribute, level / 100)));
  };
  return <div className="parameter-controls"><div className="family-tabs">{Object.keys(parameterFamilies).map((name) => <button onClick={() => setFamily(name as Family)} className={family === name ? "active" : ""} key={name}>{name}</button>)}{specialFamilies.has(family) && <button className="special-dialogs" onClick={() => dispatch({ type: "OPEN_SPECIAL_DIALOG", family: family as "Color" | "Position" | "Beam" | "Control" | "Dynamics" })}>◇ Special Dialog</button>}</div><div className="parameter-surfaces">{parameterFamilies[family].map(([name, value, level, attribute]) => <button className="touch-surface" onClick={() => void applyParameter(attribute, level)} key={name} style={{ "--level": `${level}%` } as CSSProperties}><span>{name}</span><strong>{value}</strong><small>Tap to apply to {server.selectedFixtures.length || "selected"} fixtures</small></button>)}</div></div>;
}
