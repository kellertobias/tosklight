import { useEffect, useMemo, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DmxSnapshot } from "../api/types";

interface Slot { universe: number; address: number; value: number }

export function DmxWindow({ compact }: WindowProps) {
  const server = useServer();
  const [expanded, setExpanded] = useState<number | null>(null);
  const [slot, setSlot] = useState<Slot | null>(null);
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
  const [view, setView] = useState<"values" | "sources" | "routes">("values");

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
    server.patch?.fixtures.forEach((fixture) => values.add(fixture.universe));
    server.patch?.routes.forEach((route) => values.add(route.logical_universe));
    if (!values.size) values.add(1);
    return [...values].sort((a, b) => a - b).slice(0, compact ? 2 : 8);
  }, [snapshot, server.patch, compact]);

  const fixtureFor = (universe: number, address: number) => server.patch?.fixtures.find((fixture) => fixture.universe === universe && address >= fixture.address && address < fixture.address + fixture.definition.footprint);
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
    <header className="window-toolbar"><h1>DMX Output <small>Live · diagnostic override</small></h1><span className="spacer"/><button onClick={() => setView("values")} className={view === "values" ? "active" : ""}>Values as dots</button><button onClick={() => setView("sources")} className={view === "sources" ? "active" : ""}>Sources</button><button onClick={() => setView("routes")} className={view === "routes" ? "active" : ""}>Routes</button></header>
    <div className="dmx-content"><main>{view === "values" && universeNumbers.map((universe) => {
      const frame = snapshot?.universes.find((item) => item.universe === universe);
      return <section className={`dmx-universe ${expanded === universe ? "expanded" : ""}`} key={universe}>
        <header><b>Logical universe {universe} · channels 1–512</b><button onClick={() => setExpanded(expanded === universe ? null : universe)}>{expanded === universe ? "COMPACT" : "EXPAND"}</button></header>
        {Array.from({ length: 8 }, (_, row) => <div className="dmx-row" key={row}><code>0x{(row * 64 + 1).toString(16).toUpperCase().padStart(3, "0")}</code><div>{Array.from({ length: 64 }, (_, column) => {
          const address = row * 64 + column + 1;
          const value = frame?.slots[address - 1] ?? 0;
          return <button key={address} aria-label={`Universe ${universe}, address ${address}, value ${value}`} className={value > 210 ? "high" : value > 90 ? "mid" : value > 20 ? "low" : ""} onClick={(event) => { positionInspector(event.currentTarget); setSlot({ universe, address, value }); }}/>;
        })}</div></div>)}
      </section>;
    })}{view === "sources" && <div className="dmx-detail-list"><h2>Diagnostic overrides</h2>{snapshot?.overrides.length ? snapshot.overrides.map((item) => <article key={`${item.universe}-${item.address}`}><b>Universe {item.universe} · Address {item.address}</b><span>{item.value}</span><button onClick={() => void server.setDmxOverride(item.universe, item.address, null)}>Release</button></article>) : <div className="empty-window-message">No raw DMX overrides are active.</div>}</div>}{view === "routes" && <div className="dmx-detail-list"><h2>Universe routes</h2>{server.patch?.routes.length ? server.patch.routes.map((route, index) => <article key={index}><b>Logical {route.logical_universe} → {route.protocol} {route.destination_universe}</b><span>{route.destination ?? "Multicast"}</span><small>{route.enabled ? "Enabled" : "Disabled"}</small></article>) : <div className="empty-window-message">No output routes are configured.</div>}</div>}</main>{!compact && <aside><b>Output summary</b><section>Frame rate <span>{server.bootstrap?.output_health.frame_hz.toFixed(1) ?? "—"} Hz</span></section><section>Packets <span>{server.bootstrap?.output_health.packets_sent ?? 0}</span></section><section>Errors <span>{server.bootstrap?.output_health.send_errors ?? 0}</span></section></aside>}</div>
    {slot && <div className="dmx-inspector"><button onClick={() => setSlot(null)}>×</button><b>U{slot.universe} · 0x{slot.address.toString(16).toUpperCase().padStart(3, "0")}</b><p>{fixtureFor(slot.universe, slot.address)?.definition.model ?? "Unpatched slot"}</p><label>Raw value <strong>{slot.value}</strong><input type="range" min="0" max="255" value={slot.value} onChange={(event) => override(Number(event.target.value))}/></label><button onClick={() => override(255)}>Set full</button><button onClick={() => override(null)}>Release override</button><small>Diagnostic override · session {server.session?.user.name}</small></div>}
  </div>;
}
