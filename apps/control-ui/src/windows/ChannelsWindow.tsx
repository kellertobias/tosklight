import { useEffect, useMemo, useState } from "react";
import type { VisualizationSnapshot } from "../api/types";
import { useServer } from "../api/ServerContext";
import { VerticalTouchFader } from "../components/control/VerticalTouchFader";
import type { WindowProps } from "./windowTypes";
import { fixtureValue } from "./fixtureVisualization";
import { createPortal } from "react-dom";
import { Button } from "../components/common";
import { FaderView, WindowHeader } from "../components/window-kit";
import { usePollingResource } from "../hooks/usePollingResource";
import {
  useProgrammingSelectionActions,
  useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";

const PAGE_SIZE = 20;

export function ChannelsWindow({ active = true, compact }: WindowProps) {
  const server = useServer();
  const selection = useProgrammingSelectionView(active);
  const selectionActions = useProgrammingSelectionActions(active);
  const selectedFixtureIds = useMemo(
    () => new Set(selection?.selected ?? []),
    [selection?.selected],
  );
  const [page, setPage] = useState(0);
  const [pagePickerOpen, setPagePickerOpen] = useState(false);
  const [visualization, setVisualization] = useState<VisualizationSnapshot | null>(null);
  usePollingResource({
    enabled: active,
    intervalMillis: 250,
    load: server.readVisualization,
    onValue: setVisualization,
  });
  useEffect(() => {
    if (!pagePickerOpen) return;
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); setPagePickerOpen(false); } };
    window.addEventListener("keydown", close, true);
    return () => window.removeEventListener("keydown", close, true);
  }, [pagePickerOpen]);
  const channels = server.patch?.fixtures.map((fixture, index) => ({
    number: index + 1,
    fixture,
    name: fixture.definition.name ?? fixture.definition.model,
    level: Math.round(fixtureValue(visualization, fixture, "intensity") * 100),
  })) ?? [];
  const pages = Math.max(8, Math.ceil(channels.length / PAGE_SIZE));
  const visible = Array.from({ length: PAGE_SIZE }, (_, index) => channels[page * PAGE_SIZE + index] ?? null);
  return <div className="channels-window">
    {!compact && <WindowHeader title="Channels" info={{ primary: "Intensity", secondary: "Two-row channel bank" }} actions={[[{ id: "previous", label: "←", disabled: page === 0, ariaLabel: "Previous channel page", onClick: () => setPage(page - 1) },{ id: "page", label: `${page * PAGE_SIZE + 1}–${(page + 1) * PAGE_SIZE}`, onClick: () => setPagePickerOpen(true) },{ id: "next", label: "→", disabled: page >= pages - 1, ariaLabel: "Next channel page", onClick: () => setPage(page + 1) }]]} />}
    <FaderView rows={2} className="channel-fader-bank">{visible.map((channel, index) => {
      const number = page * PAGE_SIZE + index + 1;
      return <article className={`channel-fader ${channel ? "" : "empty"} ${channel && selectedFixtureIds.has(channel.fixture.fixture_id) ? "selected" : ""}`} key={channel?.fixture.fixture_id ?? `empty-${number}`} onClick={() => channel && void selectionActions?.replace({ resolvedFixtures: [channel.fixture.fixture_id] })}>
        <VerticalTouchFader
          disabled={!channel}
          label={channel ? `CH ${number}` : `CH ${number} · Empty`}
          mode={channel?.name ?? "Unpatched"}
          value={channel?.level ?? 0}
          display={channel ? `${channel.level}%` : "—"}
          onChange={(value) => channel && void server.setProgrammer(channel.fixture.fixture_id, "intensity", value / 100)}
        />
      </article>;
    })}</FaderView>
    {pagePickerOpen && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setPagePickerOpen(false)}><div className="nested-modal channel-page-modal" role="dialog" aria-modal="true" aria-label="Channel pages"><Button className="modal-close" onClick={() => setPagePickerOpen(false)}>×</Button><h3>Channel pages</h3><div>{Array.from({ length: pages }, (_, nextPage) => <Button className={nextPage === page ? "active" : ""} key={nextPage} onClick={() => { setPage(nextPage); setPagePickerOpen(false); }}>{nextPage * PAGE_SIZE + 1}–{(nextPage + 1) * PAGE_SIZE}</Button>)}</div></div></div>, document.body)}
  </div>;
}
