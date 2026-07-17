import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DmxSnapshot, PatchedFixture } from "../api/types";
import { Button } from "../components/common";
import { useApp } from "../state/AppContext";
import { WindowHeader, WindowScrollArea, WindowSettings } from "../components/window-kit";
import { TouchValueButton } from "../components/control/VerticalTouchFader";

interface Slot { universe: number; address: number; value: number }
export interface DmxFixtureChannel {
  fixture: PatchedFixture;
  fixtureChannel: number;
  attribute: string;
  component: string | null;
}

export function fixtureChannelAt(fixtures: PatchedFixture[], universe: number, address: number): DmxFixtureChannel | null {
  for (const fixture of fixtures) {
    const patches = [{ universe: fixture.universe, address: fixture.address }, ...(fixture.multipatch ?? [])];
    const patch = patches.find((item) => item.universe === universe && item.address != null && address >= item.address && address < item.address + fixture.definition.footprint);
    if (!patch?.address) continue;
    const offset = address - patch.address;
    for (const head of fixture.definition.heads) {
      for (const parameter of head.parameters) {
        const componentIndex = parameter.components.findIndex((component) => component.offset === offset);
        if (componentIndex < 0) continue;
        const component = parameter.components.length > 1
          ? (componentIndex === 0 ? "coarse" : componentIndex === 1 ? "fine" : `byte ${componentIndex + 1}`)
          : null;
        return { fixture, fixtureChannel: offset + 1, attribute: parameter.attribute, component };
      }
    }
    return { fixture, fixtureChannel: offset + 1, attribute: "Unassigned", component: null };
  }
  return null;
}

const dipWeights = [1, 2, 4, 8, 16, 32, 64, 128, 256];
export function dmxChannelsPerRow(width: number, size: "small" | "large") {
  const target = size === "large" ? 42 : 9;
  const usable = Math.max(160, width - 72);
  return Math.max(1, Math.min(512, Math.floor((usable + 3) / (target + 3))));
}

export function DmxWindow({ compact }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [slot, setSlot] = useState<Slot | null>(null);
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
  const [view, setView] = useState<"values" | "sources">("values");
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
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

  useEffect(() => {
    if (!slot || !snapshot) return;
    const value = snapshot.universes.find((frame) => frame.universe === slot.universe)?.slots[slot.address - 1] ?? 0;
    setSlot((current) => current && current.universe === slot.universe && current.address === slot.address && current.value !== value ? { ...current, value } : current);
  }, [snapshot, slot?.universe, slot?.address]);

  const selectedFixtureChannel = slot ? fixtureChannelAt(server.patch?.fixtures ?? [], slot.universe, slot.address) : null;
  const override = (value: number | null) => {
    if (!slot) return;
    if (value !== null) setSlot({ ...slot, value });
    void server.setDmxOverride(slot.universe, slot.address, value);
  };

  return <div className="dmx-window">
    {!compact && <WindowHeader title="DMX Output" info={{ primary: "Live", secondary: "Diagnostic override" }} actions={[[{ id: "values", label: "Values as dots", active: view === "values", onClick: () => setView("values") },{ id: "sources", label: "Sources", active: view === "sources", onClick: () => setView("sources") }]]} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
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
    })}{view === "sources" && <div className="dmx-detail-list"><h2>Diagnostic overrides</h2>{snapshot?.overrides.length ? snapshot.overrides.map((item) => <article key={`${item.universe}-${item.address}`}><b>Universe {item.universe} · Address {item.address}</b><span>{item.value}</span><Button onClick={() => void server.setDmxOverride(item.universe, item.address, null)}>Release</Button></article>) : <div className="empty-window-message">No raw DMX overrides are active.</div>}</div>}</main></WindowScrollArea><aside className="dmx-info-pane">{slot ? <><header className="dmx-info-header"><b>Selected channel</b><Button size="compact" onClick={() => setSlot(null)}>Deselect</Button></header><section className="dmx-address-card"><strong>Universe {slot.universe} · Channel {slot.address}</strong><small>DMX address {slot.address} · 0x{slot.address.toString(16).toUpperCase().padStart(3, "0")}</small><div className="dmx-dip-switches" aria-label={`DIP switches for DMX address ${slot.address}`}>{dipWeights.map((weight) => <span className={slot.address & weight ? "on" : ""} key={weight}><i aria-hidden="true"/><small>{weight}</small></span>)}</div></section><section className="dmx-fixture-card"><b>Fixture</b>{selectedFixtureChannel ? <dl><dt>Fixture ID</dt><dd>{selectedFixtureChannel.fixture.fixture_number ?? selectedFixtureChannel.fixture.fixture_id}</dd><dt>Name</dt><dd>{selectedFixtureChannel.fixture.name || selectedFixtureChannel.fixture.definition.name || "—"}</dd><dt>Type</dt><dd>{selectedFixtureChannel.fixture.definition.device_type || "—"}</dd><dt>Fixture channel</dt><dd>{selectedFixtureChannel.fixtureChannel} of {selectedFixtureChannel.fixture.definition.footprint}</dd><dt>Attribute</dt><dd>{selectedFixtureChannel.attribute}{selectedFixtureChannel.component ? ` · ${selectedFixtureChannel.component}` : ""}</dd></dl> : <p>Fixture: Empty</p>}</section><div className="dmx-raw-value"><TouchValueButton label="Raw value" value={slot.value} maximum={255} display={String(Math.round(slot.value))} onChange={(value) => override(Math.round(value))}/></div><Button fullWidth onClick={() => override(null)}>Release override</Button></> : <><b>Output summary</b><section>Frame rate <span>{server.bootstrap?.output_health.frame_hz.toFixed(1) ?? "—"} Hz</span></section><section>Packets <span>{server.bootstrap?.output_health.packets_sent ?? 0}</span></section><section>Errors <span>{server.bootstrap?.output_health.send_errors ?? 0}</span></section></>}</aside></div>
  </div>;
}
