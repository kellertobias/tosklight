import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { numericPadLayout, oscProgrammerActionForKey, softwareKeyLabel, type NumericPadSection, type SoftwareKey } from "../../shared/programmerKeypad";
import { feedbackPagePlaybackOffset, oscPaths } from "./oscPaths";

type Blink = "off" | "on" | "slow" | "medium" | "fast";
type Lamp = { color: string; state: Blink; bpm?: number };
type Feedback = { address: string; arguments: unknown[] };
const dark: Lamp = { color: "#25303a", state: "off" };

function arg(value: unknown) {
  if (typeof value === "object" && value !== null) return Object.values(value as Record<string, unknown>)[0];
  return value;
}

function ControlButton({ label, lamp = dark, onDown, onUp, className = "", style, keypadKey }: { label: string; lamp?: Lamp; onDown: () => void; onUp: () => void; className?: string; style?: React.CSSProperties; keypadKey?: SoftwareKey }) {
  const timer = useRef<number | undefined>(undefined);
  const [long, setLong] = useState(false);
  const [pressed, setPressed] = useState(false);
  const release = () => { clearTimeout(timer.current); onUp(); window.setTimeout(() => setPressed(false), 90); };
  return <button className={`control-button ${lamp.state} ${lamp.state === "on" && lamp.bpm ? "beat" : ""} ${pressed ? "local-pressed" : ""} ${className}`} data-keypad-key={keypadKey} style={{ ...style, "--lamp": lamp.color, "--bpm": lamp.bpm ?? 60 } as React.CSSProperties}
    onPointerDown={(event) => { event.currentTarget.setPointerCapture(event.pointerId); setPressed(true); setLong(false); timer.current = window.setTimeout(() => setLong(true), 650); onDown(); }}
    onPointerUp={release} onPointerCancel={release}>
    <span>{label}</span>{long && <i>LONG</i>}
  </button>;
}

function WheelSafeRange(props: React.InputHTMLAttributes<HTMLInputElement>) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const range = input.current;
    if (!range) return;
    const rejectWheel = (event: WheelEvent) => { event.preventDefault(); range.blur(); };
    range.addEventListener("wheel", rejectWheel, { passive: false });
    return () => range.removeEventListener("wheel", rejectWheel);
  }, []);
  return <input {...props} ref={input} type="range"/>;
}

function TouchFader({ label, value, display, onChange, className = "" }: { label: string; value: number; display: string; onChange: (value: number) => void; className?: string }) {
  return <label className={`touch-fader ${className}`} style={{ "--fader-level": value } as React.CSSProperties}><span>{label}</span><strong>{display}</strong><WheelSafeRange min="0" max="1" step=".001" value={value} onChange={(event) => onChange(Number(event.target.value))}/></label>;
}

interface PlaybackProps { slot: number; levels: Record<number, number>; lamps: Record<string, Lamp>; send: (path: string, args: unknown[]) => void; buttons?: 1 | 3; }
function Playback({ slot, levels, lamps, send, buttons = 3 }: PlaybackProps) {
  const base = oscPaths.pagePlayback(slot);
  const button = (number: number, className = "") => <ControlButton className={className} label={String(number)} lamp={lamps[`${slot}/${number}`]}
    onDown={() => send(`${base}/button/${number}`, [true])} onUp={() => send(`${base}/button/${number}`, [false])}/>;
  return <article className={`playback buttons-${buttons}`}>
    <b>{slot}</b>{button(1, "playback-button-one")}
    <TouchFader className="playback-touch-fader" label="FADER" value={levels[slot] ?? 0} display={`${Math.round((levels[slot] ?? 0) * 100)}%`} onChange={(value) => send(`${base}/fader`, [value])}/>
    {buttons === 3 && <footer>{button(2)}{button(3)}</footer>}
  </article>;
}

function TimeFader({ label, path, maximum, send }: { label: string; path: string; maximum: number; send: (path: string, args: unknown[]) => void }) {
  const [value, setValue] = useState(0.15);
  return <TouchFader className="time-fader" label={label} value={value} display={`${(value * maximum).toFixed(1)}s`} onChange={(next) => { setValue(next); send(path, [next]); }}/>
}

function EncoderEmulation({ number, nav = false, send }: { number: number; nav?: boolean; send: (path: string, args: unknown[]) => void }) {
  const [held, setHeld] = useState(false);
  const path = nav ? oscPaths.navigation : oscPaths.encoder(number);
  return <section className={`encoder-emulation ${held ? "held" : ""}`}>
    <button aria-label={`${nav ? "Navigation" : `Encoder ${number}`} ${held ? "left" : "up"}`} onClick={() => send(path, [held ? "left" : "up"])}>{held ? "‹" : "⌃"}</button>
    <div><button onClick={() => send(path, ["press"])}>CLK</button><button className={held ? "active" : ""} onClick={() => setHeld((value) => !value)}>HLD</button></div>
    <button aria-label={`${nav ? "Navigation" : `Encoder ${number}`} ${held ? "right" : "down"}`} onClick={() => send(path, [held ? "right" : "down"])}>{held ? "›" : "⌄"}</button>
    <small>{nav ? "NAV" : number}</small>
  </section>;
}

export function App() {
  const saved = JSON.parse(localStorage.getItem("tosklight.hardware") ?? "{}");
  const [host, setHost] = useState(saved.host ?? "127.0.0.1"), [port, setPort] = useState(saved.port ?? 9000), [desk, setDesk] = useState(saved.desk ?? "main");
  const [connected, setConnected] = useState(false), [tab, setTab] = useState<"console" | "grid" | "settings">("console"), [top, setTop] = useState(saved.top ?? true), [page, setPage] = useState(1);
  const [levels, setLevels] = useState<Record<number, number>>({}), [lamps, setLamps] = useState<Record<string, Lamp>>({}), [speedBpms, setSpeedBpms] = useState<Record<number, number>>({});
  const [highlight, setHighlight] = useState({ active: false, index: 0, total: 0, fixture: "No fixture", canNext: false, canPrevious: false });
  const [updateArmed, setUpdateArmed] = useState(false);
  useEffect(() => { const off = listen<Feedback>("osc-feedback", ({ payload }) => {
    setConnected(true); const parts = payload.address.split("/"), args = payload.arguments.map(arg);
    if (payload.address.endsWith("/feedback/page")) setPage(Number(args[0]));
    if (payload.address.endsWith("/feedback/update/armed")) setUpdateArmed(Boolean(args[0]));
    const highlightOffset = parts.indexOf("highlight");
    if (highlightOffset >= 0 && parts[highlightOffset - 1] === "feedback") {
      const field = parts[highlightOffset + 1];
      if (field === "active") { const active = Boolean(args[0]); setHighlight((current) => ({ ...current, active })); setLamps((current) => ({ ...current, highlight: { color: active ? "#ffef76" : "#576069", state: active ? "on" : "off" } })); }
      if (field === "index") setHighlight((current) => ({ ...current, index: Number(args[0]) }));
      if (field === "total") setHighlight((current) => ({ ...current, total: Number(args[0]) }));
      if (field === "can-next") setHighlight((current) => ({ ...current, canNext: Boolean(args[0]) }));
      if (field === "can-previous") setHighlight((current) => ({ ...current, canPrevious: Boolean(args[0]) }));
      if (field === "fixture" && parts[highlightOffset + 2] === "name") setHighlight((current) => ({ ...current, fixture: String(args[0] || "No fixture") }));
    }
    const speedOffset = parts.indexOf("speed-group");
    if (speedOffset >= 0) { const number = Number(parts[speedOffset + 1]), bpm = Number(args[0]), state: Blink = args[4] === "off" ? "off" : "on"; const color = `rgb(${Math.round(Number(args[1]) * 255)} ${Math.round(Number(args[2]) * 255)} ${Math.round(Number(args[3]) * 255)})`; setSpeedBpms((current) => ({ ...current, [number]: bpm })); setLamps((current) => ({ ...current, [`speed/${number}`]: { color, state, bpm } })); }
    const offset = feedbackPagePlaybackOffset(parts); if (offset < 0) return; const slot = Number(parts[offset + 1]);
    if (parts[offset + 2] === "fader") setLevels((current) => ({ ...current, [slot]: Number(args[0]) }));
    if (parts[offset + 2] === "button") { const color = `rgb(${Math.round(Number(args[0]) * 255)} ${Math.round(Number(args[1]) * 255)} ${Math.round(Number(args[2]) * 255)})`; setLamps((current) => ({ ...current, [`${slot}/${parts[offset + 3]}`]: { color, state: String(args[3]) as Blink } })); }
  }); return () => { void off.then((dispose) => dispose()); }; }, []);
  const connect = async () => { setConnected(false); await invoke("connect_osc", { host, port: Number(port), desk }); localStorage.setItem("tosklight.hardware", JSON.stringify({ host, port: Number(port), desk, top })); };
  useEffect(() => { void connect(); }, []);
  const send = (path: string, args: unknown[]) => void invoke("send_control", { path, args });
  const action = (name: string, down: boolean) => send(oscPaths.programmer(name), [down]);
  const highlightAction = (name: Parameters<typeof oscPaths.highlight>[0], down: boolean) => send(oscPaths.highlight(name), [down]);
  const key = (label: string, className = "", actionName = label.toLowerCase()) => <ControlButton className={`key-${actionName === "." ? "dot" : actionName} ${className}`} label={label} onDown={() => action(actionName, true)} onUp={() => action(actionName, false)}/>;
  const renderKeypadSection = (section: NumericPadSection) => numericPadLayout
    .filter((item) => item.section === section)
    .map(({ key: keypadKey, column, row, rowSpan = 1 }) => {
      const sectionColumn = section === "commands" ? column : column - 3;
      const displayRow = row + 1;
      const actionName = oscProgrammerActionForKey(keypadKey);
      return <ControlButton
        key={keypadKey}
        keypadKey={keypadKey}
        className={`key-${actionName} ${keypadKey === "ENT" ? "key-enter" : ""}`}
        label={softwareKeyLabel(keypadKey)}
        style={{ gridColumn: sectionColumn, gridRow: `${displayRow} / span ${rowSpan}` }}
        onDown={() => action(actionName, true)}
        onUp={() => action(actionName, false)}
      />;
    });
  const speedGroups = <section className="speed-groups"><h2>Speed groups</h2>{[1, 2, 3, 4, 5].map((number) => {
    const bpm = speedBpms[number] ?? 120;
    return <div className="encoder" key={number}><ControlButton label={`SPEED ${number}`} lamp={lamps[`speed/${number}`]} onDown={() => send(oscPaths.speedGroupButton(number), [true])} onUp={() => send(oscPaths.speedGroupButton(number), [false])}/><TouchFader className="speed-touch-fader" label="RATE" value={(bpm - 1) / 998} display={`${bpm} BPM`} onChange={(value) => send(oscPaths.speedGroupEncoder(number), [Math.round(1 + value * 998)])}/></div>;
  })}</section>;

  return <main className={updateArmed ? "update-armed" : ""}><header><h1>ToskLight <span>Hardware Controls</span></h1>{updateArmed && <strong className="hardware-update-state" role="status">UPDATE ARMED · touch an assigned playback</strong>}<i className={connected ? "connected" : ""}>{connected ? `● Connected · page ${page}` : "○ Connecting…"}</i></header>
    <nav><button className={tab === "console" ? "active" : ""} onClick={() => setTab("console")}>Playback Console</button><button className={tab === "grid" ? "active" : ""} onClick={() => setTab("grid")}>Button Grid 41–90</button><button className={tab === "settings" ? "active" : ""} onClick={() => setTab("settings")}>Settings</button>{tab === "console" && <label><input type="checkbox" checked={top} onChange={(event) => { setTop(event.target.checked); localStorage.setItem("tosklight.hardware", JSON.stringify({ host, port: Number(port), desk, top: event.target.checked })); }}/> Show 21–40</label>}</nav>
    {tab === "console" ? <section className="console-layout">
      <aside className="left-rail">{key("ESCAPE")}{key("MENU")}{key("PROG-PLAYBACK")}<span className="button-spacer"/><ControlButton className="key-align" label="ALIGN" onDown={() => undefined} onUp={() => undefined}/><span className="button-spacer"/><button onClick={() => send(oscPaths.page, [Math.max(1, page - 1)])}>PAGE UP</button><strong>{page}</strong><button onClick={() => send(oscPaths.page, [page + 1])}>PAGE DOWN</button></aside>
      <section className={`playback-surface ${top ? "with-top-row" : "without-top-row"}`}><div className="encoder-row">{Array.from({ length: 6 }, (_, index) => <EncoderEmulation key={index} number={index + 1} send={send}/>)}<EncoderEmulation number={7} nav send={send}/></div><div className="top-row">{Array.from({ length: 20 }, (_, index) => index + 21).map((slot) => <ControlButton key={slot} label={String(slot)} lamp={lamps[`${slot}/1`]} onDown={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [true])} onUp={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [false])}/>)}</div>
        <div className="playback-bank">{Array.from({ length: 20 }, (_, index) => index + 1).map((slot) => <Playback key={slot} slot={slot} levels={levels} lamps={lamps} send={send}/>)}</div>
      </section>
      <aside className="programmer-panel"><div className="hardware-number-block">
        <div className="hardware-keypad-section hardware-keypad-command-section">
          <ControlButton className="key-record" label={updateArmed ? "UPDATE" : "RECORD"} lamp={updateArmed ? { color: "#f4b942", state: "on" } : dark} style={{ gridColumn: 1, gridRow: 1 }} onDown={() => action("record", true)} onUp={() => action("record", false)}/>
          <ControlButton className="key-preload-go" label="PRELOAD GO" style={{ gridColumn: 2, gridRow: 1 }} onDown={() => action("preload", true)} onUp={() => action("preload", false)}/>
          {renderKeypadSection("commands")}
        </div>
        <div className="hardware-keypad-section hardware-keypad-number-section">
          <ControlButton className="highlight-key highlight-high" label="HIGH" lamp={lamps.highlight} style={{ gridColumn: 1, gridRow: 1 }} onDown={() => highlightAction("toggle", true)} onUp={() => highlightAction("toggle", false)}/>
          <ControlButton className="highlight-key" label="PREV" lamp={highlight.canPrevious ? { color: "#68b9c7", state: "on" } : dark} style={{ gridColumn: 2, gridRow: 1 }} onDown={() => highlightAction("previous", true)} onUp={() => highlightAction("previous", false)}/>
          <ControlButton className="highlight-key" label="NEXT" lamp={highlight.canNext ? { color: "#68b9c7", state: "on" } : dark} style={{ gridColumn: 3, gridRow: 1 }} onDown={() => highlightAction("next", true)} onUp={() => highlightAction("next", false)}/>
          <ControlButton className="highlight-key" label="ALL" style={{ gridColumn: 4, gridRow: 1 }} onDown={() => highlightAction("capture", true)} onUp={() => highlightAction("capture", false)}/>
          {renderKeypadSection("numbers")}
        </div>
      </div><div className="fade-times"><TimeFader label="Prog Fade" path="programmer/prog-fade" maximum={20} send={send}/><TimeFader label="Cue Fade" path="programmer/cue-fade" maximum={60} send={send}/></div></aside>
    </section> : tab === "grid" ? <section className="grid-layout"><div className="button-grid">{Array.from({ length: 50 }, (_, index) => index + 41).map((slot) => <ControlButton key={slot} label={String(slot)} lamp={lamps[`${slot}/1`]} onDown={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [true])} onUp={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [false])}/>)}</div><aside className="grid-sidebar"><section className="six"><h2>Playbacks 91–96</h2>{Array.from({ length: 6 }, (_, index) => index + 91).map((slot) => <Playback key={slot} slot={slot} buttons={1} levels={levels} lamps={lamps} send={send}/>)}</section>{speedGroups}</aside></section> : <section className="settings"><h2>OSC connection</h2><p>The controller connects automatically when it starts. Changes are saved for the next launch.</p><label>Server<input value={host} onChange={(event) => setHost(event.target.value)}/></label><label>OSC port<input type="number" value={port} onChange={(event) => setPort(Number(event.target.value))}/></label><label>Desk alias<input value={desk} onChange={(event) => setDesk(event.target.value)}/></label><button onClick={connect}>{connected ? "Save and reconnect" : "Connect"}</button><small>{connected ? `Connected to ${desk} on ${host}:${port}` : `Connecting to ${host}:${port}…`}</small></section>}
  </main>;
}
