import { useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";

export function SpecialDialogsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [pan, setPan] = useState(50), [tilt, setTilt] = useState(50);
  const [color, setColor] = useState("#2cb7d6");
  const [beam, setBeam] = useState(50), [dynamicSpeed, setDynamicSpeed] = useState(30);
  if (!state.specialDialogsOpen) return null;
  const family = state.specialDialogFamily;
  const close = () => dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });
  const apply = async (attribute: string, value: number) => Promise.all(server.selectedFixtures.map((fixture) => server.setProgrammer(fixture, attribute, value)));
  const applyColor = async (value: string) => {
    setColor(value);
    const channels = [1, 3, 5].map((offset) => Number.parseInt(value.slice(offset, offset + 2), 16) / 255);
    await Promise.all(server.selectedFixtures.flatMap((fixture) => [server.setProgrammer(fixture, "color.red", channels[0]), server.setProgrammer(fixture, "color.green", channels[1]), server.setProgrammer(fixture, "color.blue", channels[2])]));
  };
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><section className="modal-card special-dialog-card"><button className="modal-close" onClick={close}>×</button><h2>{family} · Special Dialog</h2><p>{server.selectedFixtures.length} fixtures selected</p><div className="special-dialog-content">
    {family === "Position" && <div className="position-pad"><label>Pan <strong>{pan}%</strong><input type="range" min="0" max="100" value={pan} onChange={(event) => { const value = Number(event.target.value); setPan(value); void apply("pan", value / 100); }}/></label><label>Tilt <strong>{tilt}%</strong><input type="range" min="0" max="100" value={tilt} onChange={(event) => { const value = Number(event.target.value); setTilt(value); void apply("tilt", value / 100); }}/></label><div className="alignment-controls"><span>Align Pan</span>{(["left", "right", "center", "out"] as const).map((mode) => <button key={mode} onClick={() => void server.alignSelection("pan", mode)}>{mode}</button>)}</div></div>}
    {family === "Color" && <label className="color-touch-picker"><input aria-label="Fixture color" type="color" value={color} onChange={(event) => void applyColor(event.target.value)}/><strong>{color.toUpperCase()}</strong><small>Device-independent picker; fixture calibration is applied by the engine.</small></label>}
    {family === "Beam" && <label className="intensity-touch-slider">Beam / iris <strong>{beam}%</strong><input type="range" min="0" max="100" value={beam} onChange={(event) => { const value = Number(event.target.value); setBeam(value); void apply("iris", value / 100); }}/></label>}
    {family === "Control" && <div className="special-action-grid"><button onClick={() => void apply("control.lamp", 1)}>Lamp On</button><button onClick={() => void apply("control.lamp", 0)}>Lamp Off</button><button className="danger" onClick={() => void apply("control.reset", 1)}>Reset</button><button onClick={() => void apply("control.fan", .5)}>Fan Auto</button></div>}
    {family === "Dynamics" && <label className="intensity-touch-slider">Dynamic speed <strong>{dynamicSpeed} BPM</strong><input type="range" min="1" max="240" value={dynamicSpeed} onChange={(event) => { const value = Number(event.target.value); setDynamicSpeed(value); void apply("dynamic.speed", value / 240); }}/></label>}
  </div></section></div>;
}
