import { useEffect, useMemo, useState, type CSSProperties } from "react";
import type { DmxSnapshot } from "./api/types";
import { ServerProvider, useServer } from "./api/ServerContext";
import { NumericPad } from "./components/control/NumericPad";
import { Button, Input } from "./components/common";
import { AppShell } from "./components/shell/AppShell";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { AppProvider, useApp } from "./state/AppContext";
import { StageWindow } from "./windows/StageWindow";

const DEMO_DMX_CHANNELS = 512;
const DEMO_DMX_UNIVERSES = [1, 2] as const;

function DemoDmxGrid({ universeNumber }: { universeNumber: number }) {
  const server = useServer();
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
  useEffect(() => {
    if (server.status !== "connected") return;
    let cancelled = false;
    const refresh = () => void server.readDmx().then((next) => {
      if (!cancelled) setSnapshot(next);
    }).catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 150);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [server.status, server.readDmx]);
  const universe = snapshot?.universes.find((frame) => frame.universe === universeNumber);
  const slots = useMemo(
    () => Array.from({ length: DEMO_DMX_CHANNELS }, (_, index) => universe?.slots[index] ?? 0),
    [universe],
  );
  return <div className="product-demo-dmx-grid" aria-label={`Live DMX universe ${universeNumber}`}>
    {slots.map((value, index) => <span
      aria-label={`DMX ${universeNumber}.${index + 1}: ${value}`}
      className="product-demo-dmx-cell"
      data-address={index + 1}
      data-value={value}
      key={index}
      style={{ "--demo-dmx-level": Math.max(.07, value / 255) } as CSSProperties}
    />)}
  </div>;
}

function DemoCard({ className, title, meta, children }: { className: string; title: string; meta: string; children: React.ReactNode }) {
  return <section className={`product-demo-card ${className}`}>
    <header><b>{title}</b><span>{meta}</span></header>
    <div className="product-demo-card-body">{children}</div>
  </section>;
}

function DemoPlaybackButton({ slot, button = 1, label = String(button) }: { slot: number; button?: number; label?: string }) {
  const server = useServer();
  const [pressed, setPressed] = useState(false);
  const page = server.playbacks?.pages.find((candidate) => candidate.number === server.playbacks?.active_page);
  const playbackNumber = page?.slots[String(slot)];
  const send = (next: boolean) => {
    setPressed(next);
    if (playbackNumber == null) return;
    void server.poolPlaybackAction(playbackNumber, "button", { button, pressed: next, surface: "physical" });
  };
  return <Button
    className={`product-demo-playback-button ${pressed ? "local-pressed" : ""}`}
    aria-label={`Playback ${slot} button ${button}`}
    onPointerDown={(event) => { event.currentTarget.setPointerCapture(event.pointerId); send(true); }}
    onPointerUp={() => send(false)}
    onPointerCancel={() => send(false)}
  >{label}</Button>;
}

function DemoPlaybackStrip({ slot }: { slot: number }) {
  const server = useServer();
  const page = server.playbacks?.pages.find((candidate) => candidate.number === server.playbacks?.active_page);
  const playbackNumber = page?.slots[String(slot)];
  const active = server.playbacks?.active.find((candidate) => candidate.playback_number === playbackNumber);
  const value = Math.max(0, Math.min(1, active?.fader_position ?? active?.master ?? 0));
  return <article className="product-demo-playback-strip">
    <b>PB {slot}</b>
    <DemoPlaybackButton slot={slot}/>
    <label className="product-demo-playback-fader" style={{ "--demo-playback-level": value } as CSSProperties}>
      <span>FADER</span><strong>{Math.round(value * 100)}%</strong>
      <Input aria-label={`Playback ${slot} fader`} type="range" min="0" max="1" step=".001" value={value} onInput={(event) => {
        if (playbackNumber != null) void server.poolPlaybackAction(playbackNumber, "master", { value: Number(event.currentTarget.value), surface: "physical" });
      }}/>
    </label>
    <footer><DemoPlaybackButton slot={slot} button={2}/><DemoPlaybackButton slot={slot} button={3}/></footer>
  </article>;
}

function DemoPlaybackControls() {
  return <section className="product-demo-playbacks" aria-label="Virtual playback controls">
    <div className="product-demo-playback-top-row">{[21, 22, 23, 24].map((slot) => <DemoPlaybackButton slot={slot} label={String(slot)} key={slot}/>)}</div>
    <div className="product-demo-playback-strips">{[1, 2, 3, 4].map((slot) => <DemoPlaybackStrip slot={slot} key={slot}/>)}</div>
  </section>;
}

function ProductDemoSurface() {
  const { state, dispatch } = useApp();
  useEffect(() => {
    if (!state.midiProfile) dispatch({ type: "SET_MIDI_PROFILE", value: true });
  }, [state.midiProfile, dispatch]);
  return <main className="product-demo-shell" data-testid="product-demo">
    <section className="product-demo-application" aria-label="ToskLight application">
      <AppShell />
    </section>
    <aside className="product-demo-companion" aria-label="Virtual demo desk">
      <DemoCard className="product-demo-stage" title="STAGE · 3D" meta="SELECTION OFF · GROUPS OFF · ENV 50%">
        <StageWindow compact stageView="3d" showGroupShortcuts={false} followPreload={false} showSelection={false} environmentBrightness={.5}/>
      </DemoCard>
      <div className="product-demo-visual-divider" aria-label="Stage render above, live DMX output below"><span>⌃&nbsp; STAGE RENDER</span><span>LIVE DMX OUTPUT &nbsp;⌄</span></div>
      <DemoCard className="product-demo-dmx" title="DMX OUTPUT" meta="UNIVERSES 1–2 · LIVE">
        {DEMO_DMX_UNIVERSES.map((universeNumber) => <DemoDmxGrid universeNumber={universeNumber} key={universeNumber} />)}
      </DemoCard>
      <div className="product-demo-visual-divider" aria-label="Live DMX output above, simulated hardware controls below"><span>⌃&nbsp; LIVE DMX OUTPUT</span><span>SIMULATED HARDWARE CONTROLS &nbsp;⌄</span></div>
      <section className="product-demo-controls">
        <DemoPlaybackControls />
        <DemoCard className="product-demo-programmer" title="VIRTUAL DESK" meta="PROGRAMMER"><NumericPad demo /></DemoCard>
      </section>
    </aside>
  </main>;
}

export function ProductDemoApp() {
  return <ServerProvider>
    <AppProvider>
      <ProductDemoSurface />
    </AppProvider>
    <DeskLockOverlay />
  </ServerProvider>;
}
