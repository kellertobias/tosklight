import { useConnectionStatus } from "../features/shellStatus/ShellStatusState";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { WindowProps } from "./windowTypes";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import { useOutputHealth } from "../features/deskSnapshot/DeskSnapshotState";
import { useDmxDiagnostics } from "../features/dmxDiagnostics/DmxDiagnosticsContext";
import type {
  DmxSnapshot,
  FixtureDefinition,
  FixtureMode,
  MultiPatchInstance,
  OutputHealth,
  OutputRoute,
  PatchedFixture,
  SplitPatch,
  VersionedObject,
} from "../api/types";
import { Button } from "@tosklight/ui";
import { useApp } from "../state/AppContext";
import { WindowHeader, WindowScrollArea, WindowSettings } from "@tosklight/ui/window-kit";
import { TouchValueButton } from "@tosklight/ui/faders";
import { channelSplit, derivePrimarySlots } from "../components/setup/fixtureProfileModel";
import { usePollingResource } from "../hooks/usePollingResource";

function hz(value: number | undefined): string {
  return value === undefined || !Number.isFinite(value) || value <= 0 ? "—" : value.toFixed(1);
}

/// The desk never asks for more than this, so a faster measured frame is reported as an overflow
/// rather than as a rate an operator could act on.
const FRAME_RATE_CEILING_HZ = 60;

/// The maximum reading, capped at the fastest rate the desk is asked about.
function maximumHz(value: number | undefined): string {
  const reading = hz(value);
  if (reading === "—") return reading;
  return value !== undefined && value > FRAME_RATE_CEILING_HZ
    ? `> ${FRAME_RATE_CEILING_HZ} Hz`
    : `${reading} Hz`;
}

function OutputSummary({ health }: { health: OutputHealth | null }) {
  const seconds = health?.recent_window_seconds ?? 60;
  const bounds = health?.recent_frame_rate_bucket_bounds_hz ?? [];
  const counts = health?.recent_frame_rate_bucket_counts ?? [];
  const busiest = Math.max(1, ...counts);
  return <>
    <b>Output summary</b>
    <section className="dmx-output-rate">
      <b>Frame rate · last {seconds} s</b>
      <dl>
        <div><dt>Min</dt><dd>{hz(health?.recent_frame_hz_minimum)} Hz</dd></div>
        <div><dt>Avg</dt><dd>{hz(health?.recent_frame_hz_average)} Hz</dd></div>
        <div><dt>Max</dt><dd>{maximumHz(health?.recent_frame_hz_maximum)}</dd></div>
      </dl>
    </section>
    <section className="dmx-output-histogram">
      <b>Frames at rate · last {seconds} s</b>
      {/* The bands are disjoint, so a frame lands in exactly one row and the counts sum to the
          frames in the window: below the lowest bound, one band between each pair of bounds, and
          an overflow band above the highest. */}
      {bounds.length ? <ul>{["below", ...bounds].map((bound, index) => {
        const count = counts[index] ?? 0;
        const next = bounds[index];
        const label = index === 0
          ? <>&lt; {bounds[0]} Hz</>
          : next === undefined
            ? <>&gt; {bound} Hz</>
            : <>{bound}–{next} Hz</>;
        return <li key={index === 0 ? "below" : bound}>
          <small>{label}</small>
          <i aria-hidden="true" style={{ "--dmx-histogram-fill": `${Math.round((count / busiest) * 100)}%` } as CSSProperties}/>
          <span>{count}</span>
        </li>;
      })}</ul> : <p>No output frames recorded yet.</p>}
    </section>
    <section className="dmx-output-errors">
      <b>Errors</b>
      <dl>
        <div><dt>Last {seconds} s</dt><dd>{health?.recent_send_errors ?? 0}</dd></div>
        <div><dt>Since show start</dt><dd>{health?.send_errors ?? 0}</dd></div>
      </dl>
    </section>
  </>;
}

interface Slot { universe: number; address: number; value: number }
export interface DmxSelection { universe: number; address: number }
export interface DmxFixtureChannel {
  fixture: PatchedFixture;
  fixtureChannel: number;
  attribute: string;
  component: string | null;
  split: number;
  splitFootprint: number;
  patchOwner: { kind: "fixture" | "multipatch"; id: string; name: string };
  patchRange: { universe: number; start: number; end: number };
}

interface DmxPatchBinding {
  split: number;
  footprint: number;
  universe: number;
  address: number;
  owner: DmxFixtureChannel["patchOwner"];
}

function definitionMode(definition: FixtureDefinition): FixtureMode | null {
  const profile = definition.profile_snapshot;
  return profile?.modes.find((candidate) => candidate.id === definition.mode_id)
    ?? profile?.modes.find((candidate) => candidate.name === definition.mode)
    ?? profile?.modes[0]
    ?? null;
}

function ownerBindings(
  fixture: PatchedFixture,
  owner: { universe: number | null; address: number | null; split_patches?: SplitPatch[] },
  patchOwner: DmxFixtureChannel["patchOwner"],
): DmxPatchBinding[] {
  const mode = definitionMode(fixture.definition);
  const splits = mode?.splits.length ? mode.splits : [{ number: 1, footprint: fixture.definition.footprint }];
  const configured = new Map((owner.split_patches ?? []).map((patch) => [patch.split, patch]));
  return splits.flatMap((split, index) => {
    const patch = configured.get(split.number) ?? {
      split: split.number,
      universe: index === 0 ? owner.universe : null,
      address: index === 0 ? owner.address : null,
    };
    if (patch.universe == null || patch.address == null) return [];
    return [{ split: split.number, footprint: split.footprint, universe: patch.universe, address: patch.address, owner: patchOwner }];
  });
}

/** Every physical DMX range owned by a logical fixture, including independent splits and multi-patches. */
export function fixtureDmxPatchBindings(fixture: PatchedFixture): DmxPatchBinding[] {
  const primary = ownerBindings(fixture, fixture, { kind: "fixture", id: fixture.fixture_id, name: "Fixture patch" });
  const multipatches = (fixture.multipatch ?? []).flatMap((instance: MultiPatchInstance, index) => ownerBindings(fixture, instance, {
    kind: "multipatch",
    id: instance.id,
    name: instance.name?.trim() || `Multi-patch ${index + 1}`,
  }));
  return [...primary, ...multipatches];
}

export function fixtureChannelAt(fixtures: readonly PatchedFixture[], universe: number, address: number): DmxFixtureChannel | null {
  for (const fixture of fixtures) {
    const patch = fixtureDmxPatchBindings(fixture).find((item) => item.universe === universe && address >= item.address && address < item.address + item.footprint);
    if (!patch) continue;
    const offset = address - patch.address;
    const common = {
      fixture,
      fixtureChannel: offset + 1,
      split: patch.split,
      splitFootprint: patch.footprint,
      patchOwner: patch.owner,
      patchRange: { universe: patch.universe, start: patch.address, end: patch.address + patch.footprint - 1 },
    };
    const mode = definitionMode(fixture.definition);
    if (mode) {
      const primarySlots = derivePrimarySlots(mode).slots;
      for (const channel of mode.channels) {
        if (channelSplit(mode, channel) !== patch.split) continue;
        const slots = [primarySlots.get(channel.id) ?? 1, ...channel.secondary_slots];
        const componentIndex = slots.findIndex((slot) => slot === offset + 1);
        if (componentIndex < 0) continue;
        const component = slots.length > 1
          ? (componentIndex === 0 ? "coarse" : componentIndex === 1 ? "fine" : componentIndex === 2 ? "third byte" : "fourth byte")
          : null;
        return { ...common, attribute: channel.attribute, component };
      }
      return { ...common, attribute: "Unassigned", component: null };
    }
    for (const head of fixture.definition.heads) {
      for (const parameter of head.parameters) {
        const componentIndex = parameter.components.findIndex((component) => component.offset === offset);
        if (componentIndex < 0) continue;
        const component = parameter.components.length > 1
          ? (componentIndex === 0 ? "coarse" : componentIndex === 1 ? "fine" : `byte ${componentIndex + 1}`)
          : null;
        return { ...common, attribute: parameter.attribute, component };
      }
    }
    return { ...common, attribute: "Unassigned", component: null };
  }
  return null;
}

const dipWeights = [1, 2, 4, 8, 16, 32, 64, 128, 256];
export function dmxChannelsPerRow(width: number, size: "small" | "large") {
  const target = size === "large" ? 42 : 9;
  const usable = Math.max(160, width - 72);
  return Math.max(1, Math.min(512, Math.floor((usable + 3) / (target + 3))));
}

export function DmxWindow({ active = true, compact }: WindowProps) {
  const dmx = useDmxDiagnostics();
  const bootstrapHealth = useOutputHealth();
  const connectionStatus = useConnectionStatus();
  const { state, dispatch } = useApp();
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
  const [liveHealth, setLiveHealth] = useState<OutputHealth | null>(null);

  usePollingResource({
    enabled: active && connectionStatus === "connected",
    intervalMillis: 250,
    load:
      dmx?.readDmx ??
      (async () => ({ revision: 0, universes: [], overrides: [] })),
    onValue: setSnapshot,
  });

  // The bootstrap copy of output health is refetched only when a show opens, so the summary polls
  // the runtime for the live counters instead of reporting the state at show-open time forever.
  usePollingResource({
    enabled: active && connectionStatus === "connected" && Boolean(dmx?.readOutputHealth),
    intervalMillis: 1_000,
    load: dmx?.readOutputHealth ?? (async () => null),
    onValue: setLiveHealth,
  });

  const patchedFixtures = usePatchedFixturesView(active);

  return <DmxWindowView
    compact={compact}
    dotSize={state.dmxDotSize}
    onDotSizeChange={(value) => dispatch({ type: "SET_DMX_DOT_SIZE", value })}
    onSetDmxOverride={(universe, address, value) => dmx?.setDmxOverride(universe, address, value)}
    outputHealth={liveHealth ?? bootstrapHealth}
    outputRoutes={dmx?.outputRoutes ?? []}
    patchedFixtures={patchedFixtures}
    snapshot={snapshot}
  />;
}

export interface DmxWindowViewProps {
  compact?: boolean;
  snapshot: DmxSnapshot | null;
  patchedFixtures: readonly PatchedFixture[];
  outputRoutes: readonly VersionedObject<OutputRoute>[];
  outputHealth: OutputHealth | null;
  dotSize: "small" | "large";
  onDotSizeChange: (value: "small" | "large") => void;
  onSetDmxOverride: (universe: number, address: number, value: number | null) => void | Promise<void>;
  defaultView?: "values" | "sources";
  defaultSelection?: DmxSelection | null;
}

export function DmxWindowView({
  compact,
  snapshot,
  patchedFixtures,
  outputRoutes,
  outputHealth,
  dotSize,
  onDotSizeChange,
  onSetDmxOverride,
  defaultView = "values",
  defaultSelection = null,
}: DmxWindowViewProps) {
  const initialValue = defaultSelection
    ? snapshot?.universes.find((frame) => frame.universe === defaultSelection.universe)?.slots[defaultSelection.address - 1] ?? 0
    : 0;
  const [slot, setSlot] = useState<Slot | null>(() => defaultSelection ? { ...defaultSelection, value: initialValue } : null);
  const [view, setView] = useState<"values" | "sources">(defaultView);
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
  const targetDot = dotSize === "large" ? 42 : 9;
  const channelsPerRow = dmxChannelsPerRow(valuesWidth, dotSize);

  const universeNumbers = useMemo(() => {
    const values = new Set(snapshot?.universes.map((frame) => frame.universe) ?? []);
    patchedFixtures.forEach((fixture) => fixtureDmxPatchBindings(fixture).forEach((patch) => values.add(patch.universe)));
    outputRoutes.forEach((route) => values.add(route.body.logical_universe));
    if (!values.size) values.add(1);
    return [...values].sort((a, b) => a - b);
  }, [snapshot, patchedFixtures, outputRoutes]);

  useEffect(() => {
    if (!slot || !snapshot) return;
    const value = snapshot.universes.find((frame) => frame.universe === slot.universe)?.slots[slot.address - 1] ?? 0;
    setSlot((current) => current && current.universe === slot.universe && current.address === slot.address && current.value !== value ? { ...current, value } : current);
  }, [snapshot, slot?.universe, slot?.address]);

  const selectedFixtureChannel = slot ? fixtureChannelAt(patchedFixtures, slot.universe, slot.address) : null;
  const override = (value: number | null) => {
    if (!slot) return;
    if (value !== null) setSlot({ ...slot, value });
    void onSetDmxOverride(slot.universe, slot.address, value);
  };

  return <div className="dmx-window">
    {!compact && <WindowHeader title="DMX Output" info={{ primary: "Live", secondary: "Diagnostic override" }} groups={[{ id: "dmx-view", kind: "tabs", activeId: view, onActiveChange: (id) => setView(id as "values" | "sources"), actions: [{ id: "values", label: "Values" }, { id: "sources", label: "Sources" }] }]} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
    {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title="DMX Settings" onClose={() => setSettingsAnchor(null)} tabs={[{ id: "display", label: "Display", content: <><h3>DMX dot size</h3><div className="button-group"><Button className={dotSize === "small" ? "active" : ""} onClick={() => onDotSizeChange("small")}>Small</Button><Button className={dotSize === "large" ? "active" : ""} onClick={() => onDotSizeChange("large")}>Large</Button></div><small>{channelsPerRow} values per row at this window size</small></> }]} />}
    <div className="dmx-content"><WindowScrollArea><main ref={valuesHost} style={{ "--dmx-columns": channelsPerRow, "--dmx-dot-size": `${targetDot}px` } as CSSProperties}>{view === "values" && universeNumbers.map((universe) => {
      const frame = snapshot?.universes.find((item) => item.universe === universe);
      return <section className={`dmx-universe dots-${dotSize}`} key={universe}>
        <header><b>Logical universe {universe} · channels 1–512</b><small>{channelsPerRow} per row</small></header>
        {Array.from({ length: Math.ceil(512 / channelsPerRow) }, (_, row) => <div className="dmx-row" key={row}><code>0x{(row * channelsPerRow + 1).toString(16).toUpperCase().padStart(3, "0")}</code><div>{Array.from({ length: channelsPerRow }, (_, column) => {
          const address = row * channelsPerRow + column + 1;
          const value = frame?.slots[address - 1] ?? 0;
          if (address > 512) return null;
          return <Button key={address} aria-label={`Universe ${universe}, address ${address}, value ${value}`} className={`${value > 210 ? "high" : value > 90 ? "mid" : value > 20 ? "low" : ""} ${slot?.universe === universe && slot.address === address ? "selected" : ""}`} onClick={() => setSlot({ universe, address, value })}/>;
        })}</div></div>)}
      </section>;
    })}{view === "sources" && <div className="dmx-detail-list"><h2>Diagnostic overrides</h2>{snapshot?.overrides.length ? snapshot.overrides.map((item) => <article key={`${item.universe}-${item.address}`}><b>Universe {item.universe} · Address {item.address}</b><span>{item.value}</span><Button onClick={() => void onSetDmxOverride(item.universe, item.address, null)}>Release</Button></article>) : <div className="empty-window-message">No raw DMX overrides are active.</div>}</div>}</main></WindowScrollArea><aside className="dmx-info-pane">{slot ? <><header className="dmx-info-header"><b>Selected channel</b><Button size="compact" onClick={() => setSlot(null)}>Deselect</Button></header><section className="dmx-address-card"><strong>Universe {slot.universe} · Channel {slot.address}</strong><small>DMX address {slot.address} · 0x{slot.address.toString(16).toUpperCase().padStart(3, "0")}</small><div className="dmx-dip-switches" aria-label={`DIP switches for DMX address ${slot.address}`}>{dipWeights.map((weight) => <span className={slot.address & weight ? "on" : ""} key={weight}><i aria-hidden="true"/><small>{weight}</small></span>)}</div></section><section className="dmx-fixture-card"><b>Fixture</b>{selectedFixtureChannel ? <dl><dt>Fixture ID</dt><dd>{selectedFixtureChannel.fixture.fixture_number ?? selectedFixtureChannel.fixture.fixture_id}</dd><dt>Name</dt><dd>{selectedFixtureChannel.fixture.name || selectedFixtureChannel.fixture.definition.name || "—"}</dd><dt>Type</dt><dd>{selectedFixtureChannel.fixture.definition.device_type || "—"}</dd><dt>Patch owner</dt><dd>{selectedFixtureChannel.patchOwner.name}</dd><dt>Patch range</dt><dd>{selectedFixtureChannel.patchRange.universe}.{selectedFixtureChannel.patchRange.start}–{selectedFixtureChannel.patchRange.end}</dd><dt>Split</dt><dd>{selectedFixtureChannel.split}</dd><dt>Fixture channel</dt><dd>{selectedFixtureChannel.fixtureChannel} of {selectedFixtureChannel.splitFootprint}</dd><dt>Attribute</dt><dd>{selectedFixtureChannel.attribute}{selectedFixtureChannel.component ? ` · ${selectedFixtureChannel.component}` : ""}</dd></dl> : <p>Fixture: Empty</p>}</section><div className="dmx-raw-value"><TouchValueButton label="Raw value" value={slot.value} maximum={255} display={String(Math.round(slot.value))} onChange={(value) => override(Math.round(value))}/></div><Button fullWidth onClick={() => override(null)}>Release override</Button></> : <OutputSummary health={outputHealth}/>}</aside></div>
  </div>;
}
