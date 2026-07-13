import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DmxSnapshot } from "../api/types";
import { Button, Input } from "../components/common";
import { useApp } from "../state/AppContext";
import { WindowHeader, WindowScrollArea, WindowSettings } from "../components/window-kit";
import { ModalNumberInput } from "../components/input/ModalInputControls";

interface Slot { universe: number; address: number; value: number }
export function dmxChannelsPerRow(width: number, size: "small" | "large") {
  const target = size === "large" ? 42 : 9;
  const usable = Math.max(160, width - 72);
  return Math.max(1, Math.min(64, Math.floor((usable + 3) / (target + 3))));
}

export function DmxWindow({ compact }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [slot, setSlot] = useState<Slot | null>(null);
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
  const [view, setView] = useState<"values" | "sources" | "routes">("values");
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
  const [valueInputOpen, setValueInputOpen] = useState(false);
  const [valueInput, setValueInput] = useState("");
  const valuesHost = useRef<HTMLElement>(null);
  const [valuesWidth, setValuesWidth] = useState(900);

  useEffect(() => {
    const host = valuesHost.current;
    if (!host) return;
    const observer = new ResizeObserver(([entry]) => setValuesWidth(entry.contentRect.width));
    observer.observe(host);
    return () => observer.disconnect();
  }, [view]);
  const targetDot = state.dmxDotSize === "large" ? 42 : 9;
  const channelsPerRow = dmxChannelsPerRow(valuesWidth, state.dmxDotSize);

  useEffect(() => {
    if (server.status !== "connected") return;
    let cancelled = false;
    const poll = () => void server.readDmx().then((next) => { if (!cancelled) setSnapshot(next); }).catch(() => undefined);
    poll();
    const timer = window.setInterval(poll, 250);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [server.status]);

  const universeNumbers = useMemo(() => {
    const values = new Set(snapshot?.universes.map((frame) => frame.universe) ?? []);
    server.patch?.fixtures.forEach((fixture) => { if (fixture.universe != null) values.add(fixture.universe); });
    server.patch?.routes.forEach((route) => values.add(route.logical_universe));
    if (!values.size) values.add(1);
    return [...values].sort((a, b) => a - b).slice(0, compact ? 2 : 8);
  }, [snapshot, server.patch, compact]);

  const fixtureFor = (universe: number, address: number) => server.patch?.fixtures.find((fixture) => [{universe:fixture.universe,address:fixture.address}, ...(fixture.multipatch ?? [])].some((patch) => patch.universe === universe && patch.address != null && address >= patch.address && address < patch.address + fixture.definition.footprint));
  const override = (value: number | null) => {
    if (!slot) return;
    if (value !== null) setSlot({ ...slot, value });
    void server.setDmxOverride(slot.universe, slot.address, value);
  };

  return <div className="dmx-window">
    {!compact && <WindowHeader title="DMX Output" info={{ primary: "Live", secondary: "Diagnostic override" }} actions={[[{ id: "values", label: "Values as dots", active: view === "values", onClick: () => setView("values") },{ id: "sources", label: "Sources", active: view === "sources", onClick: () => setView("sources") },{ id: "routes", label: "Routes", active: view === "routes", onClick: () => setView("routes") }]]} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
    {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title="DMX Settings" onClose={() => setSettingsAnchor(null)} tabs={[{ id: "display", label: "Display", content: <><h3>DMX dot size</h3><div className="button-group"><Button className={state.dmxDotSize === "small" ? "active" : ""} onClick={() => dispatch({type:"SET_DMX_DOT_SIZE",value:"small"})}>Small</Button><Button className={state.dmxDotSize === "large" ? "active" : ""} onClick={() => dispatch({type:"SET_DMX_DOT_SIZE",value:"large"})}>Large</Button></div><small>{channelsPerRow} values per row at this window size</small></> }]} />}
    <div className="dmx-content"><WindowScrollArea><main ref={valuesHost} style={{ "--dmx-columns": channelsPerRow, "--dmx-dot-size": `${targetDot}px` } as CSSProperties}>{view === "values" && universeNumbers.map((universe) => {
      const frame = snapshot?.universes.find((item) => item.universe === universe);
      return <section className={`dmx-universe dots-${state.dmxDotSize}`} key={universe}>
        <header><b>Logical universe {universe} · channels 1–512</b><small>{channelsPerRow} per row</small></header>
        {Array.from({ length: Math.ceil(512 / channelsPerRow) }, (_, row) => <div className="dmx-row" key={row}><code>0x{(row * channelsPerRow + 1).toString(16).toUpperCase().padStart(3, "0")}</code><div>{Array.from({ length: channelsPerRow }, (_, column) => {
          const address = row * channelsPerRow + column + 1;
          const value = frame?.slots[address - 1] ?? 0;
          if (address > 512) return null;
          return <Button key={address} aria-label={`Universe ${universe}, address ${address}, value ${value}`} className={`${value > 210 ? "high" : value > 90 ? "mid" : value > 20 ? "low" : ""} ${slot?.universe === universe && slot.address === address ? "selected" : ""}`} onClick={() => setSlot({ universe, address, value })}/>;
        })}</div></div>)}
      </section>;
    })}{view === "sources" && <div className="dmx-detail-list"><h2>Diagnostic overrides</h2>{snapshot?.overrides.length ? snapshot.overrides.map((item) => <article key={`${item.universe}-${item.address}`}><b>Universe {item.universe} · Address {item.address}</b><span>{item.value}</span><Button onClick={() => void server.setDmxOverride(item.universe, item.address, null)}>Release</Button></article>) : <div className="empty-window-message">No raw DMX overrides are active.</div>}</div>}{view === "routes" && <div className="dmx-detail-list"><h2>Universe routes</h2>{server.patch?.routes.length ? server.patch.routes.map((route, index) => <article key={index}><b>Logical {route.logical_universe} → {route.protocol} {route.destination_universe}</b><span>{route.destination ?? "Multicast"}</span><small>{route.enabled ? "Enabled" : "Disabled"}</small></article>) : <div className="empty-window-message">No output routes are configured.</div>}</div>}</main></WindowScrollArea><aside className="dmx-info-pane">{slot ? <><b>Selected channel</b><section><strong>U{slot.universe} · {slot.address}</strong><small>0x{slot.address.toString(16).toUpperCase().padStart(3, "0")}</small><p>{fixtureFor(slot.universe, slot.address)?.definition.model ?? "Unpatched slot"}</p></section><Button className="dmx-value-input" onClick={() => { setValueInput(String(slot.value)); setValueInputOpen(true); }}><small>Raw value</small><strong>{slot.value}</strong></Button><Button onClick={() => override(null)}>Release override</Button></> : <><b>Output summary</b><section>Frame rate <span>{server.bootstrap?.output_health.frame_hz.toFixed(1) ?? "—"} Hz</span></section><section>Packets <span>{server.bootstrap?.output_health.packets_sent ?? 0}</span></section><section>Errors <span>{server.bootstrap?.output_health.send_errors ?? 0}</span></section></>}</aside></div>
    {valueInputOpen && slot && <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setValueInputOpen(false)}><section className="nested-modal direct-value-modal" role="dialog" aria-modal="true" aria-label="DMX channel value"><Button className="modal-close" onClick={() => setValueInputOpen(false)}>×</Button><h3>U{slot.universe} · Channel {slot.address}</h3><strong>{valueInput || "0"}</strong><ModalNumberInput value={valueInput} onChange={setValueInput} onEnter={() => { override(Math.max(0, Math.min(255, Number(valueInput) || 0))); setValueInputOpen(false); }} onEscape={() => setValueInputOpen(false)} replaceOnFirstInput/></section></div>}
  </div>;
}
