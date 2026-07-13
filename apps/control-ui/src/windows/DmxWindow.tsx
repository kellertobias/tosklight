import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DmxSnapshot } from "../api/types";
import { Button, Input } from "../components/common";
import { useApp } from "../state/AppContext";

interface Slot { universe: number; address: number; value: number }
export function dmxChannelsPerRow(width: number, size: "small" | "large") {
  const target = size === "large" ? 28 : 18;
  const usable = Math.max(160, width - 72);
  return ([64, 32, 16, 8] as const).find((columns) => usable / columns >= target * .85) ?? 8;
}

export function DmxWindow({ compact }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [slot, setSlot] = useState<Slot | null>(null);
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
  const [view, setView] = useState<"values" | "sources" | "routes">("values");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const valuesHost = useRef<HTMLElement>(null);
  const [valuesWidth, setValuesWidth] = useState(900);

  useEffect(() => {
    const host = valuesHost.current;
    if (!host) return;
    const observer = new ResizeObserver(([entry]) => setValuesWidth(entry.contentRect.width));
    observer.observe(host);
    return () => observer.disconnect();
  }, [view]);
  const targetDot = state.dmxDotSize === "large" ? 28 : 18;
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
  const positionInspector = (element: HTMLElement) => {
    const rect = element.getBoundingClientRect();
    document.documentElement.style.setProperty("--dmx-tip-x", `${Math.min(innerWidth - 260, rect.right + 10)}px`);
    document.documentElement.style.setProperty("--dmx-tip-y", `${Math.min(innerHeight - 190, rect.top)}px`);
  };
  const override = (value: number | null) => {
    if (!slot) return;
    if (value !== null) setSlot({ ...slot, value });
    void server.setDmxOverride(slot.universe, slot.address, value);
  };

  return <div className="dmx-window">
    <header className="window-toolbar"><h1>DMX Output <small>Live · diagnostic override</small></h1><span className="spacer"/><Button onClick={() => setView("values")} className={view === "values" ? "active" : ""}>Values as dots</Button><Button onClick={() => setView("sources")} className={view === "sources" ? "active" : ""}>Sources</Button><Button onClick={() => setView("routes")} className={view === "routes" ? "active" : ""}>Routes</Button><Button className={settingsOpen ? "active" : ""} onClick={() => setSettingsOpen((open) => !open)}>Settings</Button></header>
    {settingsOpen && <section className="dmx-settings"><header><b>DMX dot size</b><Button onClick={() => setSettingsOpen(false)}>×</Button></header><div className="button-group"><Button className={state.dmxDotSize === "small" ? "active" : ""} onClick={() => dispatch({type:"SET_DMX_DOT_SIZE",value:"small"})}>Small</Button><Button className={state.dmxDotSize === "large" ? "active" : ""} onClick={() => dispatch({type:"SET_DMX_DOT_SIZE",value:"large"})}>Large</Button></div><small>{channelsPerRow} values per row at this window size</small></section>}
    <div className="dmx-content"><main ref={valuesHost} style={{ "--dmx-columns": channelsPerRow, "--dmx-dot-size": `${targetDot}px` } as CSSProperties}>{view === "values" && universeNumbers.map((universe) => {
      const frame = snapshot?.universes.find((item) => item.universe === universe);
      return <section className={`dmx-universe dots-${state.dmxDotSize}`} key={universe}>
        <header><b>Logical universe {universe} · channels 1–512</b><small>{channelsPerRow} per row</small></header>
        {Array.from({ length: Math.ceil(512 / channelsPerRow) }, (_, row) => <div className="dmx-row" key={row}><code>0x{(row * channelsPerRow + 1).toString(16).toUpperCase().padStart(3, "0")}</code><div>{Array.from({ length: channelsPerRow }, (_, column) => {
          const address = row * channelsPerRow + column + 1;
          const value = frame?.slots[address - 1] ?? 0;
          return <Button key={address} aria-label={`Universe ${universe}, address ${address}, value ${value}`} className={value > 210 ? "high" : value > 90 ? "mid" : value > 20 ? "low" : ""} onClick={(event) => { positionInspector(event.currentTarget); setSlot({ universe, address, value }); }}/>;
        })}</div></div>)}
      </section>;
    })}{view === "sources" && <div className="dmx-detail-list"><h2>Diagnostic overrides</h2>{snapshot?.overrides.length ? snapshot.overrides.map((item) => <article key={`${item.universe}-${item.address}`}><b>Universe {item.universe} · Address {item.address}</b><span>{item.value}</span><Button onClick={() => void server.setDmxOverride(item.universe, item.address, null)}>Release</Button></article>) : <div className="empty-window-message">No raw DMX overrides are active.</div>}</div>}{view === "routes" && <div className="dmx-detail-list"><h2>Universe routes</h2>{server.patch?.routes.length ? server.patch.routes.map((route, index) => <article key={index}><b>Logical {route.logical_universe} → {route.protocol} {route.destination_universe}</b><span>{route.destination ?? "Multicast"}</span><small>{route.enabled ? "Enabled" : "Disabled"}</small></article>) : <div className="empty-window-message">No output routes are configured.</div>}</div>}</main>{!compact && <aside><b>Output summary</b><section>Frame rate <span>{server.bootstrap?.output_health.frame_hz.toFixed(1) ?? "—"} Hz</span></section><section>Packets <span>{server.bootstrap?.output_health.packets_sent ?? 0}</span></section><section>Errors <span>{server.bootstrap?.output_health.send_errors ?? 0}</span></section></aside>}</div>
    {slot && <div className="dmx-inspector"><Button onClick={() => setSlot(null)}>×</Button><b>U{slot.universe} · 0x{slot.address.toString(16).toUpperCase().padStart(3, "0")}</b><p>{fixtureFor(slot.universe, slot.address)?.definition.model ?? "Unpatched slot"}</p><label>Raw value <strong>{slot.value}</strong><Input type="range" min="0" max="255" value={slot.value} onChange={(event) => override(Number(event.target.value))}/></label><Button onClick={() => override(255)}>Set full</Button><Button onClick={() => override(null)}>Release override</Button><small>Diagnostic override · session {server.session?.user.name}</small></div>}
  </div>;
}
